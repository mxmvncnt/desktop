# Architecture

## Workspace layout

```
.
├── Cargo.toml          workspace; GPUI-CE pinned by git rev
├── crates/
│   ├── git/   (gd-git)  UI-agnostic git layer, shells out to `git`
│   ├── syntax/ (gd-syntax) UI-agnostic tree-sitter highlighting
│   ├── ui/    (gd-ui)   zeron-style design system: theme, motion, components, assets
│   └── app/   (desktop-rs)  the binary: models + views
├── docs/
└── .ref/ target/   (git-ignored; see below)
```

`.ref/` holds read-only reference clones: `.ref/zeron` (the design source) and
`.ref/gpui-ce` (the framework source at our pinned rev). Read those instead of
guessing APIs. To recreate them:
`git clone --depth 1 https://github.com/zeronsh/zeron .ref/zeron` and the same for `gpui-ce/gpui-ce`.

## Git layer (`crates/git`)

- Like Desktop's `dugite`, it **shells out to the system `git`** (`exec.rs`).
  Hooks, credential helpers, LFS, config, and SSH all behave as they do on the CLI.
  It sets `LC_ALL=C`, `GIT_OPTIONAL_LOCKS=0`, and `core.quotepath=false`.
  Network ops set `GIT_TERMINAL_PROMPT=0`, so credentials must come from a helper or ssh-agent.
  (An auth UI is still to do.)
- It only parses machine formats: `status --porcelain=v2 -z`,
  `log -z` with `%x1f` separators, `for-each-ref` with `%00`, `show --name-status -z`.
- **All functions block.** Call them only from a background executor.
- Commit model (`Repository::commit`): reset the index to HEAD, `add -A` the
  checked paths, then `commit -F -`. This matches Desktop's checkbox semantics.
  Partial files go through `commit_partial` / `amend_commit(.., partial)`, see §Partial staging.
- Diffs are parsed into a flat `Vec<DiffLine>` (hunk headers included) so the UI
  can index row `i` in O(1) from a `uniform_list`. Parsing is capped at 50k lines (`truncated`).
- `examples/smoke.rs` exercises the write paths against a scratch repo:
  `cargo run -p gd-git --example smoke -- /tmp/desktop-rs/smoke`.

## Stashes (`gd-git/src/stash.rs`, `app/src/features/stashes/`)

- Listing: `git stash list -z --format=%H%x1f%h%x1f%ct%x1f%cd%x1f%gs` with `--date=format-local`. `%gs` is
  `WIP on <branch>: <sha> <subject>` (no user message) or `On <branch>: <msg>`; `parse_subject` extracts branch/message
  (`(no branch)` = detached). Stashes are identified by sha, never by index: write ops re-resolve `stash@{n}` from the sha
  right before running (indices shift when others are dropped).
- Layout: tracked changes = `git diff S^1 S`; untracked files (`-u`) live in the parentless third parent `S^3`
  (`ls-tree -r` for the list, `git show S^3 -- path` for the diff). Untracked entries get `FileStatus::Untracked`.
- Push uses `-u` always; pathspec pushes pass `--literal-pathspecs` (paths like `notes [1].md`). Gotcha: the global
  `--literal-pathspecs` breaks `-u` cleanup when no pathspec is given, so it is only passed with paths.
- `exec.rs` now falls back to stdout for the error text when stderr is empty (`stash apply` prints CONFLICT lines on stdout).
- Rename = `stash store -m <new subject> <sha>` then drop the old entry (moves to top).
- State: `RepoModel::stashes: StashState` (`features/stashes/state.rs`, ops in `ops.rs`; list, filter, collapsed, selection, files/diff/highlight, generation counter);
  loaded in the background from every full refresh. `RepoEvent::SwitchBranchWithChanges` makes `RepoModel::checkout`
  hand dirty-tree checkouts to the Workspace, which owns `Workspace::stash_ui` (dialog + message input). Use
  `RepoModel::checkout_now` to skip the dialog. `gd_ui::Dialog::content` embeds inputs/option rows.
- `select_change` clears `stashes.viewing`, which is what swaps the main pane between stash detail and the working diff.

## Diff engine (`crates/git/src/engine.rs`, the `similar` crate)

- git only _fetches_ the two versions (`Side::{Empty, Blob{rev,path}, Worktree(path)}` via
  `Repository::read_side`: `cat-file blob rev:path`, or the filesystem). The diff is computed
  in-process (`engine::diff_texts`: Myers via `capture_diff_slices_deadline` with an 800ms deadline,
  `group_diff_ops(…, 3)` for hunks). Working, commit and stash diffs all go through
  `Repository::diff_sides`.
- Hide whitespace = compare lines with all whitespace removed (git `-w` semantics). A missing
  final newline is part of the line key, so newline-only changes show git's "\ No newline" row.
- Word emphasis: in each replace block the i-th deleted row pairs with the i-th added row.
  Lines are tokenized into word/space/punct tokens and diffed again, and changed byte ranges go to
  `DiffLine::emphasis`. It's skipped above 1000 chars or when more than 60% of both lines changed. It's
  rendered as a background wash by `diff_highlight::line_text` (merged with the syntax runs).
- `Diff::source` keeps the exact old/new texts (for partial staging). It's `None` when either side
  isn't valid UTF-8, so staging never writes back lossily decoded bytes. Binary = NUL in the first 8000
  bytes, like git. `too_large` above 8 MB per side.
- Cross-check vs git: `cargo run --release -p gd-git --example diff_compare -- <repo> [N]`.
  On this repo's last 150 commits, 468/472 files had identical +/- counts. The rest differ where git's
  Myers heuristics are non-minimal (e.g. yarn.lock, where ours is smaller).

## Partial staging (`gd-git/src/partial.rs`, `app/src/line_selection.rs`, `views/diff.rs`)

- No `git apply`: `staged_text(diff, include)` replays the diff over `DiffSource::old` (raw lines via
  `split_inclusive`, so CRLF and a missing final newline survive) keeping only included adds/deletes.
  `stage_partial` checks the worktree still equals `source.new` (else errors: "changed on disk"), then
  `hash-object -w --stdin --path=<p>` + `update-index --add --cacheinfo <mode>,<sha>,<p>` (mode = worktree exec bit
  on unix, else HEAD's). Renames also `--force-remove` the old path.
- App state: `RepoModel.partial: HashMap<path, LineSelection { diff: Arc<Diff>, excluded: HashSet<row> }>` next to
  `excluded` (whole file off). `set_rows_included` normalizes: all rows off → `excluded`, none off → entry removed, so
  `partial` only holds mixed files. A reloaded diff keeps the selection only if `lines` are identical
  (`reconcile_selection`); `-w` diffs are ignored there. `partial` is cleared after a successful commit/amend.
- UI: `render_diff(.., select: Option<Entity<RepoModel>>, ..)`. `views/diff.rs` exports `number_cell` (one cell
  holding 1 or 2 numbers; plain, or clickable/drag-extending with the blue+check included style) and
  `RowSelect::handle_cell` (one cell of a change block's handle), shared by the unified list and the split list's
  middle gutter. Handles work on change blocks (`RepoModel::change_block`: the add/delete run around a row), not git
  hunks. Pointer state (`hovered_handle: (block start, cell)`, `drag` anchor) is `Workspace.diff_select`, reset when
  the diff key changes. The cell id in `hovered_handle` makes leave/enter order between two cells of one block
  harmless. Drag ends via `on_mouse_up_all` on the list wrapper and only extends while `pressed_button == Left`.
- Hover states can be captured headlessly: `GD_SNAPSHOT_MOUSE=x,y` (window px) makes `--snapshot` dispatch a
  MouseMove there before capturing (`hover_style` and `on_hover` both react).
- Colors: number cells `gutter_bg()` (ink 2.5%), idle handle strip `handle_bg()` (black 60%, visibly darker),
  unselected block handle ink 10%; included line cells `line_selected_bg()` (blue-700, hover `accent_strong`),
  included/partial blocks `accent_strong` (hover `accent`).
- Gotcha: `on_mouse_up_all` takes 5 args (event, phase, hitbox, window, cx).

## Hunk expansion (`gd-git/src/expand.rs`, `views/diff.rs` `Expander`)

- `DiffHunk` carries 0-based `old_start/old_len/new_start/new_len`. `expand(diff, Up(h)|Down(h)|All(h))` is pure: it
  splits the flat rows into hunks, pulls context rows from `source.new` into the gap (20 = `EXPANSION_STEP`), merges
  touching hunks, re-flattens with fresh headers. Changed rows are copied untouched, so `staged_text` gives the same
  result before and after expanding (tested). `gap_above`/`gap_below` drive which buttons show.
- `RepoModel::expand_diff(commit, e)` swaps `change_diff`/`commit_diff` and re-highlights in the background.
  `change_diff_key` (path + options) lets `load_change_diff` keep the expanded diff when a reload returns the same
  source texts. Line selection is keyed by `LineKey = (is_add, line number)` instead of row index, so it survives
  row shifts; `reconcile_selection` drops it only when the source texts change.
- `render_diff(.., pane: DiffPane, ..)`: `Changes` (select + expand), `History` (expand), `ReadOnly` (stashes).
  Header rows render `Expander::gutter` instead of number cells; an extra trailing list row (index == rows) holds
  the last Expand Down. Buttons use `group` + `group_hover_style` so the svg icon recolors on hover.

## Threading / state (`crates/app`)

- `Workspace` (`workspace.rs`) is the root view. It owns settings, popover state,
  text-input entities, scroll handles, and an `Option<Entity<RepoModel>>`.
- `RepoModel` (`repo.rs`) owns all repository state: status, selection, diff,
  history pages, commit details, branches, and in-flight ops.
  Every git call goes through `cx.background_spawn`, and results are applied with `this.update`.
- **Stale results are dropped** with generation counters (`change_diff_gen`,
  `commit_gen`). Clicking quickly through files never shows the wrong diff.
- `RepoModel::run(op, on_ok)` is the generic write-op helper. It runs off-thread,
  sets `error`, then refreshes status and branches.
- Refresh triggers: file watcher, window activation (skipped if the watcher
  refreshed <1s ago), after every op, F5. See §File watcher.
- Views live in `views/*.rs` as `impl Workspace` blocks. Rows in lists hold
  `Entity<RepoModel>` clones and call `update` in click handlers.

### Menus, dialogs, undo (`views/menus.rs`)

- `Workspace.menu` / `Workspace.dialog` hold the open state. While either is open the root gets key
  context `"Workspace Menu"` and focus, so `MenuNext/MenuPrevious/MenuConfirm` (↑/↓/Enter) and
  `Dismiss` (Esc) reach the workspace. Menu rows are described once in `rows(target)`, used for both
  rendering and keyboard stepping.
- Right-click handlers in `uniform_list` rows capture `cx.entity()` of the Workspace and call
  `ws.update(cx, ..)` with the row index, resolved against the live model at event time.
- `RepoModel.last_commit` records the session's commit; `undoable_commit()` hides it when the branch
  differs. Undo emits `RepoEvent::CommitUndone`, which the Workspace subscribes to in order to refill the inputs.
- Setting Workspace fields directly (snapshot code) needs `cx.notify()`.

### Performance rules (primary goal)

1. Never run git on the UI thread.
2. Any list that can be long must be a `uniform_list`: file lists, history, diffs, pickers.
3. History is paged (`HISTORY_PAGE` = 200). More pages load as the list nears its end.
4. Animations are one-shot. Don't use `repeat()` animations for idle UI. The only
   repeating one is the toolbar busy pulse, and it exists only during network ops.
   (zeron's `motion.rs` notes that a repeating animation pins the window at 120 Hz.)
5. The dev profile builds deps at `opt-level=3` so debug runs are representative.
6. Release profile: thin LTO, `codegen-units=1`.

Measuring idle cost: `GD_DEBUG_FRAMES=1` logs `renders=N moves=M` once per second
while the root view renders (`app/src/frame_debug.rs`). Silence means truly idle.
Renders far above pointer moves mean something requests frames on its own.
Don't measure with raw CPU ticks on a live desktop: pointer movement over the
window legitimately re-renders (hover transitions), which made one session chase
a phantom regression.

Known perf TODOs: split `Workspace` into child entities so typing in the commit
box doesn't re-render the diff (it's virtualized, so it's cheap but not free);
add syntax highlighting with a cache; add a file watcher so focus refresh isn't the only trigger.

## File watcher and refresh coalescing

- `RepoModel::new` starts `RepoWatcher` (`watcher.rs`): one OS thread owning the
  `notify` watcher; blocks on an mpsc channel when idle (no polling). It does a
  trailing debounce (300ms quiet, 2s max) and sends one tick (`bool` = git
  metadata changed) over a `smol` channel to a foreground task in the model.
  Dropping the model drops `RepoWatcher`, which sends `Stop`; the thread exits and the watches go away.
- Watches: root non-recursive, each top-level dir recursively (except `.git`,
  `target`, `node_modules`), git dir non-recursive + `refs/` recursive. New
  top-level dirs get a watch on their Create event. Not one recursive root watch:
  that would cover `.git/objects` and build dirs.
- **Gotcha:** inotify reports `Access(Open)` for reads, and our own `git status`
  opens every file and `.git/index`. Only Create/Modify/Remove/Any events count,
  otherwise refresh retriggers itself forever. `GIT_OPTIONAL_LOCKS=0` keeps status from writing the index.
- `RepoModel::request_refresh(full, cx)` is the only refresh path: if one is in
  flight it records `pending_refresh` and exactly one follow-up runs afterwards
  (`full` is OR-ed). Status-only refreshes (worktree edits) skip branches/history.
  `refresh_count` + a debug log line per refresh make loops visible.
- Last fetch = mtime of `<common git dir>/FETCH_HEAD` (read in each refresh).
  Background fetch task: first check at 10s, then wakes every 60s (also re-renders
  the relative-time caption); fetches when both last fetch and last attempt are >=30 min old.
  Failures are only logged.
- Headless test (`--snapshot ... watch`): the watcher debounces in wall-clock
  time, so use real sleeps and poll `run_until_parked`.

## History operations (`gd_git::history_ops`, `app/src/history_ops.rs`, `views/history_menu.rs`)

- State: `RepoModel.history_ops` (unpushed set, tags by sha, GitHub web URL, in-progress op, busy label,
  multi-selection as shas) reloaded in the background by `reload_push_state` on every full refresh.
  UI state: `Workspace.history_ui` (menu, dialogs, amend, dialog text inputs). Own menu/dialog types so the
  Changes menu code is untouched; `workspace.rs` routes Esc/Enter/arrows to it first.
- Every op goes through `RepoModel::hist_run`, a thin wrapper over the merge branch's `run_op` (one at a time, then `refresh`). Menu items are
  disabled while an op, remote op, commit, or stopped merge/rebase/cherry-pick/revert is active.
- Unpushed: `rev-list @{upstream}..HEAD`, or `HEAD --not --remotes` without upstream; empty with no remotes.
- Squash: adjacent, linear commits only. The squashed commit is built with `commit-tree` (newest tree, oldest's
  parent). If the run ends at HEAD, `update-ref` moves the branch (working tree and index untouched).
  Otherwise `rebase --autostash --onto <new> <newest>` replays the later commits; they apply cleanly
  because the tree is unchanged.
- Reorder: `move_commit` (swap with neighbour) writes a todo file into `.git/gd-rebase-todo` and runs
  `rebase -i` with `-c sequence.editor="cp '<file>'"`; removed afterwards. No drag and drop (uniform_list
  drag with autoscroll not attempted); menu "Move commit up/down" instead.
- Revert/cherry-pick/squash-rebase/reorder return `OpOutcome` via `outcome_of`; on conflicts the op stays in progress and the generic in-progress banner (Continue/Abort) takes over. No private abort helpers.
- Amend: `amend_commit` resets the index, adds the checked paths, `commit --amend --allow-empty -F -`.
  Undo reuses `undo_last_commit` after seeding `last_commit` from HEAD, so the message returns via `CommitUndone`.
- Gotchas: `Focusable` must be imported for `Entity<EditableTextState>::focus_handle`; `gd_ui::tooltip(text)` is the
  tooltip builder (element needs an id); `Dialog::content` takes custom content (inputs, lists).

## Merge, rebase and conflicts

- `gd-git`: `progress.rs` (`Repository::in_progress() -> Option<InProgress>`: Merge/Squash/Rebase/CherryPick/Revert, read from MERGE_HEAD, `rebase-merge/`, `rebase-apply/`, CHERRY_PICK_HEAD, REVERT_HEAD, SQUASH_MSG+unmerged; `continue_operation`, `abort_operation`, `prepared_message`) and `merge.rs` (`merge`, `merge_squash`, `rebase`, `merge_preview`, `commit_merge`, `resolve_with`, `mark_resolved`, branch rename/delete-remote/create-from). Operations return `OpOutcome::{Done, UpToDate, Conflicts}`; a pause is not an error. `Git::no_editor()` sets GIT_EDITOR=true for `--continue`.
- The watcher already covers `rebase-*` (git dir top level) and MERGE_HEAD etc.
- App: `RepoModel.merge: MergeState` (`merge_ops.rs`), loaded in every refresh in the background. `RepoModel::run_op` is the busy-flag write helper. A newly paused merge-like op emits `RepoEvent::PreparedMessage` (fills the commit box if empty). While merge-like ops are paused, `commit` uses `commit_merge` (no index reset).
- Views: `views/in_progress_banner.rs` (banner + conflict hint above the diff), `views/merge.rs` (`Workspace.merge_ui`: merge dialog, branch/conflict context menus, rename/delete/create dialogs). Conflicted file rows route to the conflict menu in `open_file_menu`.
- **Hooking another op (revert, cherry-pick, squash) in:** run the git command inside `RepoModel::run_op`, map a pause with `Repository::outcome_of`, and use `notice_for_outcome`. Banner, conflict list and Continue/Abort work automatically.
- Gotchas: absolute overlays must themselves be `.absolute().inset_0()` (GPUI lays absolute children out against their direct parent); a focused text input turns ↑/↓/Esc into `NavUp/NavDown/Escape` actions, so dialogs use `capture_action` for those; `gen` is a reserved word in edition 2024.

## Syntax highlighting (`crates/syntax`, `app/src/diff_highlight.rs`)

- `gd-syntax` is ported from zeron's `zeron-syntax`, trimmed to Rust, JS/JSX/TS/TSX, Python,
  Go, JSON, Bash, TOML, Markdown, HTML (+JS/CSS injection), CSS, YAML, C, C++.
  `highlight(HighlightRequest{source, path, ..})` returns per-line `(byte_range, HighlightKind)`.
  Limits: 2 MB source, 1M spans; unknown extension -> `UnknownLanguage`.
- Why tree-sitter over syntect: parse is incremental-grade fast (18k rows in ~125 ms), queries
  are shared with zeron, and the palette maps 1:1 to `HighlightKind`. Cost: ~16 grammar crates
  of C code (longer cold build, a few MB of binary).
- `gd-ui::theme::SyntaxPalette` (zeron dark: indigo keywords/functions, emerald strings,
  amber numbers/types, pink macros). Variable/parameter/operator/punctuation are drawn in the
  row's own color (no run) to cut noise and run count.
- App flow: after a diff loads, `RepoModel::spawn_highlight` runs `highlight_diff` via
  `background_spawn`. Until it lands the view renders plain text; then `change_hl`/`commit_hl`
  is set and the row switches to `StyledText::with_runs` (same metrics, so no layout shift).
  Staleness: the result is applied only if `Arc::ptr_eq` with the currently displayed diff;
  the task slot is overwritten on each new diff. Results are cached in an 8-entry LRU keyed by
  hash(path, diff text), shared with the background task.
- Both sides are highlighted separately (context+delete = old, context+add = new; context and
  adds take new-side spans, deletes old-side spans). Hunks are stitched with a blank line, so a
  comment/string spanning an unchanged gap may be mis-colored (full-file highlight would fix it).
- Caps: skip above 20k diff rows or 2 MB text. Per-row runs are built only for visible rows.
- Gotcha: `StyledText::with_runs` panics if run
  lengths don't sum to the text length / split a char; `runs_for` guards both.
- Gotcha: `Hsla` here is palette `Alpha<Hsl>`: use `color.color.saturation`, not `.s`.
- Scroll reset: `Workspace::diff_key` stores the last diff identity; a change resets the list's
  `base_handle` offset and `DiffView::hscroll` to 0.

## Diff options and split view (`views/diff_options.rs`, `views/diff_split.rs`)

- Options are app-wide and persisted in `Settings` (`diff_split`, `diff_ignore_whitespace`, `diff_wrap`), stored in a
  separate `display` file (`key=value`) so old `repositories` files load unchanged.
- `gd_git::DiffOptions { ignore_whitespace }` is passed to `working_diff`/`commit_diff` (`-w`).
  `RepoModel::diff_options` holds it; `set_diff_options` reloads the visible change/commit diff via the
  normal generation counters (old diff stays on screen until the new one lands). `Workspace::apply_diff_options` pushes settings in;
  `open_repository` seeds new models.
- `Workspace::diff_view: DiffView` holds the popover position and the cached `SplitDiff`
  (keyed by `Arc::ptr_eq` on the diff). `SplitDiff::build` is one linear pass producing
  `SplitRow { left, right }` index pairs into `Diff::lines` (`NONE` = filler): context on both sides, hunk
  headers span both columns (`left == right`), delete/add runs are paired row by row and padded.
  Rendering is a single list (id `diff-split`); each row is `[old text | middle gutter | new text]`.
  Syntax spans reuse `DiffHighlight::row(ix)` unchanged.
- Long lines (`views/diff.rs`, `TextFlow`): both modes share `unified_row`/`split_list` row renderers and
  `rows_list`, which picks the list type.
  - Scroll (default): a vertical-only `uniform_list`. Horizontal scroll is done by hand: each text cell
    (`text_cell`) clips and shifts its text by `DiffView::hscroll`. Gutters stay fixed and both split
    columns always move together. `diff_body`'s `on_scroll_wheel` takes the x delta (shift+wheel already
    arrives as x from the platform) and calls `Workspace::scroll_diff_x`. That clamps the offset using the
    shaped width of `diff.longest_line` and `HExtent` (non-text width + column count, set each render).
    The `uniform_list` gets `restrict_scroll_to_axis` (set via `interactivity().base_style`, since
    UniformList isn't `StatefulInteractiveElement`); otherwise a y-only scroller turns x deltas into
    vertical scrolling.
  - Wrap ("Wrap long lines"): a variable-height `gpui::list` on `DiffView::wrap_list`. `wrap_list_for`
    resets it when the diff `Arc` or split mode changes, and keeps the top row on same-key reloads. The
    list re-measures on width change by itself. Rows use `min_h`, and text uses `whitespace_normal`.
  - Gotcha: no `h_full` on gutter cells. A percentage height can't resolve in an auto-height (wrapped)
    row and blocks flex stretch. The cells stretch instead, are top-aligned, and keep check marks in a
    one-line-tall box.
- The popover is a `deferred` overlay like `ContextMenu` (closes on backdrop click/Esc; counted in `overlay_open`).
- Gotcha: a `SplitRow` side can be `NONE`; never index `lines[left]` before checking `left == right`.
- Snapshot views set settings fields directly (no file writes).

## UI kit (`crates/ui`)

- `theme.rs`: zeron's dark tokens, verbatim (oklch → sRGB helper included).
  Use `Theme::of(cx)`, `ink(a)` for translucent white washes, and `hairline(a)` for lines.
- Themes: `Theme::dark()` / `Theme::light()`; `Workspace::apply_theme` resolves the setting and does
  `cx.set_global(Theme::for_mode(light))`. `ink()`/`hairline()` are free functions (no `cx`), so they read a
  global flag set by `for_mode` (white washes on dark, black on light). Never hardcode `white()`/`black()`;
  use theme tokens or `ink()`, and check new UI with both `changes` and `changes-light` snapshots.
- `motion.rs`: the zeron motion catalog (`fade_in` 500ms expo-out with 4px rise,
  `fade_quick`, `menu_in`, `dialog_in`, plus an exact `CubicBezier`).
- `components.rs`: `pressable` (row/ghost base with transitioned hover wash), `Button`
  (Primary, Secondary, Ghost, Danger), `Checkbox`, `Tabs` (segmented), `ToolbarButton`
  (Desktop's toolbar dropdowns), `badge`, `overlay_card`, `divider`/`vdivider`, `icon`.
- `overlay.rs`: `ContextMenu` (positioned at the mouse, clamped to the window, `menu_in`; items with
  icon/danger/disabled, separators, keyboard highlight via `.selected(ix)`) and `Dialog` (scrim + `dialog_in` card,
  Cancel + primary/danger confirm). Both are stateless `RenderOnce` wrapping a `deferred` full-window layer;
  the owner renders them only while open and handles keys.
- `assets.rs`: embedded Solar icons and Geist fonts. To add an icon, copy the
  SVG from `.ref/zeron/crates/ui/assets/icons/` into `crates/ui/assets/icons/`
  and add a line to the `icons!` macro.

## Platform code (`app/src/platform/`)

OS-specific code lives in `platform/linux/` and `platform/macos/`; `platform/mod.rs`
re-exports the one for the target. Both expose the same surface: `Chrome`,
`caption_buttons`, `titlebar_drag`, `frame`, `configure_window`,
The config dir comes from the `dirs` crate (`dirs::config_dir()`) in `settings.rs`.
and the shortcut constants `SECONDARY` (ctrl/cmd), `SECONDARY_LABEL`, `REFRESH_KEY`,
`QUIT_KEY`. Keep OS `cfg`s out of the rest of the app; add to both modules instead.
Wayland/X11 gpui features are enabled only for Linux targets in `crates/app/Cargo.toml`.

- **macOS** (not yet compiled or run, no Mac available): native frame with a transparent
  titlebar, so the toolbar is the titlebar. `Chrome::leading_inset` (78px) clears the traffic
  lights, the empty toolbar space is a `WindowControlArea::Drag`, no caption buttons or CSD
  frame. Shortcuts use cmd (Refresh is cmd-r, Quit cmd-q); config is in
  `~/Library/Application Support/desktop-rs`.

## Window chrome / CSD (`app/src/platform/linux/chrome.rs`)

- Linux requests `WindowDecorations::Client` with `LinuxWindowBackground::Transparent`.
  GNOME/Wayland never draws a frame. If the compositor negotiates server
  decorations instead (KDE, X11 WMs), `Chrome::detect` reports `client=false` and nothing extra renders.
- Floating: a 12px transparent margin (`set_client_inset`) holds the shadow, then a
  10px-rounded, hairline-bordered window. Tiled/maximized: no margin, square corners.
- GPUI content masks are rectangles (`.rounded()` doesn't clip children), so every
  layer touching a window corner rounds its own background: the toolbar (`rounded_t`),
  the workspace root, and the sidebar (`rounded_bl`, via `Workspace::corner_radius`).
- The titlebar is the toolbar's empty flex spacer (`titlebar_drag`). It isn't the whole
  toolbar, because mouse-down would bubble from the buttons and start a window move.
  Caption buttons stop propagation and follow `cx.button_layout()` (left and/or right).
- Window-coordinate positions (mouse events) must subtract `window.client_inset()`
  before absolutely positioning inside the frame (see `ContextMenu`).
- `GD_SNAPSHOT_CSD=1` forces the floating CSD frame in `--snapshot` renders.

## GPUI-CE gotchas (learned the hard way)

- **`AssetSource::list("")` must enumerate every asset.** GPUI-CE builds its
  `AssetRegistry` from it once at startup. If an asset isn't listed, `svg()`
  silently draws nothing.
- Colors are `palette` types. `.opacity()` needs `use gpui::ColorExt`. Build
  colors with `gpui::rgb_to_hsla(gpui::Rgba::new(r,g,b,a))`, not struct literals.
- `with_animation` needs `use gpui::AnimationExt`.
- Style transitions: `.transitions(|t| t.bg(millis(120)).text_color(..))` plus
  `.hover_style(..)` / `.active_style(..)`. Transitionable properties include
  `bg`, `border_color`, `text_color`, sizes, insets, and ring.
  `.hover(..)` and `.group_hover(..)` are deprecated aliases of the `_style` forms.
- `.flex_shrink(f32)` takes an argument.
- `uniform_list(id, n, |range, window, cx| ..)` has no entity parameter. Capture an `Entity` clone.
- Edition 2024: `-> impl IntoElement` on `&mut self` methods captures the borrow.
  Use `-> impl IntoElement + use<>` (only when no APIT args), or return `AnyElement`.
- Text inputs: `gpui_elements::editable_text::{text_input, text_area, EditableTextState}`.
  The state is an entity (`as_str()`, `emplace()`). Subscribe to `TextChanged` to re-render.
  Bind `default_bindings()` for `DEFAULT_INPUT_CONTEXT`. Catch Enter with
  `.capture_action(|_: &editable_text::actions::Enter, ..|)` on a wrapper.
- Headless snapshots run on a fake dispatcher clock, but **animations use wall-clock time**.
  `snapshot.rs` sleeps for real and refreshes before capturing. Without that,
  fades are captured at opacity 0.
- `import`/`grim` aren't available on the dev machine. Use `--snapshot` to see the UI.

## GPUI-CE pin

`Cargo.toml` pins `rev = 9a82fba4…` (2026-10-18). To bump it, change the rev
in all four entries, run `cargo update -p gpui-ce`, rebuild, and re-check the
gotchas above.
