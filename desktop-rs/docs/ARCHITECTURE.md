# Architecture

## Workspace layout

```
desktop-rs/
├── Cargo.toml          workspace; GPUI-CE pinned by git rev
├── env.sh              source before cargo: CARGO_HOME/TMPDIR inside the project
├── crates/
│   ├── git/   (gd-git)  UI-agnostic git layer, shells out to `git`
│   ├── syntax/ (gd-syntax) UI-agnostic tree-sitter highlighting
│   ├── ui/    (gd-ui)   zeron-style design system: theme, motion, components, assets
│   └── app/   (github-desktop)  the binary: models + views
├── docs/
└── .ref/ .cargo-home/ .tmp/ target/   (git-ignored; see below)
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
  Partial-line commits aren't supported yet.
- Diffs are parsed into a flat `Vec<DiffLine>` (hunk headers included) so the UI
  can index row `i` in O(1) from a `uniform_list`. Parsing is capped at 50k lines (`truncated`).
- `examples/smoke.rs` exercises the write paths against a scratch repo:
  `cargo run -p gd-git --example smoke -- $PWD/.tmp/smoke`.

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
- Gotcha: `uniform_list` measures ONE item (default row 0) for the horizontal extent; use
  `with_width_from_item(Some(diff.longest_line))`. Gotcha: `StyledText::with_runs` panics if run
  lengths don't sum to the text length / split a char; `runs_for` guards both.
- Gotcha: `Hsla` here is palette `Alpha<Hsl>`: use `color.color.saturation`, not `.s`.
- Scroll reset: `Workspace::diff_key` stores the last diff identity; a change resets the list's
  `base_handle` offset to 0.

## UI kit (`crates/ui`)

- `theme.rs`: zeron's dark tokens, verbatim (oklch → sRGB helper included).
  Use `Theme::of(cx)`, `ink(a)` for translucent white washes, and `hairline(a)` for lines.
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
