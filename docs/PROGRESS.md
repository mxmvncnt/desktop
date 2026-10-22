# Progress

## Session 2 (2026-10-20, evening)

- Merged in: CSD window chrome, diff options (split view, hide whitespace), multiple stashes,
  merge/squash/rebase + in-progress banner + conflict resolution, and history ops
  (push status, full commit context menu, multi-select squash, amend, reorder via menu).
- All git smoke suites pass on the integrated tree: `smoke`, `merge_smoke`, `stash_smoke`, `history_ops`.
- Idle is still zero renders (`GD_DEBUG_FRAMES=1`). RSS is ~67 MB (release, small repo).
- Still untested by hand: everything interactive in the new menus and dialogs.

## Settings modal + light/dark/system theme (2026-10-21)

- [x] Settings modal (`views/settings.rs`): two panes, a category sidebar (Appearance, Diff) and the selected page
      (`Workspace::settings_page`). Custom overlay built from `overlay_card` + scrim like `Dialog`, since `Dialog` is single-column.
      Opens from the toolbar gear, `Ctrl/Cmd+,`, macOS "Settings…"; Esc/close button/backdrop closes. `settings_open` counts as
      `overlay_open`; Esc is checked first in `Dismiss`. Diff page reuses `CheckOptionRow` and the `set_diff_*` setters.
      To add a category: extend `SettingsPage` (label/icon) and add a page fn.
- [x] Theme mode `System|Light|Dark` persisted as `theme=` in the `display` file. System follows
      `window.appearance()` and re-applies live via `observe_window_appearance`.
- Not done: identities/providers, multiple color themes (see TODO.md). Snapshot views: `settings`,
  `settings-light`, `settings-diff`, `changes-light`.

## Diff long lines (2026-10-21)

- ✅ Horizontal scrolling in unified and split views (wheel x / shift+wheel). Gutters stay fixed. In split
  view both columns share one offset, so they always scroll together.
- ✅ "Wrap long lines" checkbox in the diff options popover (persisted as `diff_wrap`). It wraps both views
  through a variable-height `list`.
- Snapshot views: `hscroll`, `split-hscroll` (offset preset to 160px), `wrap`, `split-wrap`. Test repo:
  `/tmp/desktop-rs/longlines` (a single `main.rs` with long changed lines).
- Not yet tried by hand: real wheel/trackpad horizontal scrolling and the clamp at the right edge
  (snapshots can't send wheel events).

## Status at end of session 1 (2026-10-20, overnight autonomous run)

- `rust-rewrite` holds the POC plus three merged feature branches (fs-watcher,
  changes-actions, diff-syntax). Everything builds warning-free in dev, snapshot and release.
- Measured (release, Linux/Wayland, small repo): **~65 MB RSS, 0 CPU ticks over
  10 s idle**. The highlighter takes ~125 ms on an 18k-line diff, in the background.
- **Not yet exercised with a real mouse/keyboard** (agents can only render
  snapshots): right-click menus, ↑/↓/Enter in pickers and menus, text input editing,
  clipboard, folder dialog, clone, push/pull against a real remote, and horizontal diff scrolling.
  These are the first things to try by hand.
- Coordinator workflow that worked: build the foundation solo, then launch Sonnet subagents in
  `.worktrees/<name>` (branch `rust/<name>`), then merge with `--no-ff`
  and resolve the predictable conflicts (`repo.rs` fields, `snapshot.rs` view list, docs).

Branch: `rust-rewrite`. Each entry: ✅ done · 🟡 partial · ⬜ todo.

## POC (session 1, 2026-10-20)

### Foundation

- ✅ Cargo workspace (`gd-git`, `gd-ui`, `desktop-rs`), GPUI-CE pinned.
- ✅ zeron theme tokens, motion catalog, Geist fonts, Solar icons.
- ✅ Components: Button, Checkbox, Tabs, ToolbarButton, pressable rows, overlay card, badge.
- ✅ Headless `--snapshot` PNG renderer for visual verification.

### Git operations (`gd-git`, all verified by `examples/smoke.rs`)

- ✅ open / init / clone
- ✅ status (porcelain v2: branch, upstream, ahead/behind, renames, conflicts, untracked)
- ✅ working diff (tracked, untracked, unborn HEAD) and commit diff
- ✅ commit selected files (checkbox semantics) with summary and description
- ✅ discard changes (tracked and untracked); undo last commit (soft reset); `ignore` (append to `.gitignore`, `escape_ignore` helper).
- ✅ branches list, checkout (local, or remote → tracking branch), create, delete
- ✅ fetch / pull / push, plus publish (`--set-upstream`)
- ✅ paged log, commit file list

### UI

- ✅ Desktop layout: toolbar (repo ▾, branch ▾, contextual Publish/Pull/Push/Fetch), sidebar tabs, main diff.
- ✅ Changes list: checkboxes, select-all (mixed state), status glyphs, ↑/↓.
- ✅ Commit box: summary, description, `Commit to <branch>` (Ctrl+Enter), enabled only when valid.
- ✅ Virtualized diff with gutters, hunk headers, add/del washes, truncation notice.
- ✅ History: infinite-scroll commit list, commit header, file list, diff.
- ✅ Repository picker: filter, ↑/↓/Enter, remove (hover ×), add existing (folder dialog), clone.
- ✅ Branch picker: filter, local + remote (deduped), ↑/↓/Enter checkout, new branch (sanitized name).
- ✅ No-changes view (push/pull/open-folder suggestions), welcome screen.
- ✅ Error/notice toast (fade-in, notices auto-dismiss after 3s).
- ✅ Refresh on window focus, F5, and after every operation.
- ✅ Persisted repository list and last selection.

## Live updates (fs-watcher branch)

- ✅ `notify` watcher per open repo (`app/src/watcher.rs`), 300ms trailing debounce (2s max wait), coalesced refresh.
- ✅ `.git` filtering: only HEAD/index/refs/FETCH_HEAD/MERGE_HEAD/... count; objects, logs, `*.lock` ignored. `target/` (top level) and `node_modules` skipped.
- ✅ Idle = zero refreshes, ~zero CPU. Verify: `cargo run --features snapshot -- --snapshot x.png <repo> watch` (asserts; mutates and restores files/a branch in the repo).
- ✅ `gd_git::Repository::{git_dir, common_git_dir, last_fetch_time}`; toolbar caption "Last fetched 5 minutes ago" / "Never fetched".
- ✅ Background fetch: 10s after open if stale, then every 30 min; silent on failure (logged), skipped if a remote op is running or no remote.
- ✅ Focus refresh skipped if the watcher refreshed within 1s.
- Notes: headless test shows ~1s from last write to updated status (300ms debounce + dispatcher wake-up latency); real sleeps + `run_until_parked` polling are required. Debug log: `RUST_LOG=desktop_rs=debug` prints one line per refresh, `trace` prints fs events.

## Next rungs of the ladder (suggested order)

1. ✅ Changes context menu: discard (with confirm dialog), copy path, reveal, ignore. Undo-last-commit UI.
   - Right-click a file row: Discard changes… / Ignore file / Ignore all .ext files / Copy (relative) path / Reveal in file manager. Right-click the "N changed files" header: Discard all changes….
   - "Ignore file" is disabled for tracked files (ignoring them has no effect in git). Untracked files are deleted permanently on discard (Desktop moves them to the trash; no trash support yet).
   - Undo strip shows only for the commit made this session, on the same branch. Undo restores summary and description into the inputs.
   - Snapshot views: `context-menu`, `header-menu`, `discard-dialog`, `undo-strip`.
   - Not done: Discard of individual hunks, "Open in editor" menu items.
2. ✅ File-system watcher, "Last fetched" caption, background fetch (see below).
3. ✅ Syntax highlighting in diffs (background, cached). `gd-syntax` (tree-sitter), see ARCHITECTURE. Also: diff scroll resets on file/commit change, horizontal scroll sized from the widest row, hunk headers italic/muted with a divider. Timing: 18k-row added Rust file highlights in ~125 ms (small diffs 5-25 ms, first one pays ~20 ms query compile). Snapshot views `changes@N` select the N-th file.
4. ⬜ Auth: GIT_ASKPASS helper → in-app credential dialog; GitHub sign-in (OAuth device flow).
5. ✅ Multiple stashes + switch-branch-with-changes dialog (branch `rust/stashes`).
   - Sidebar "Stashes (N)" panel between the file list and commit box: collapsible (state kept per session), "This branch / All branches" filter (default this branch), newest first, this-branch vs other-branch styling. Hidden when the repo has no stashes.
   - Click a stash: main pane shows header (message, branch, date, sha) with Apply / Restore (pop) / Discard (confirm), file list + syntax-highlighted diff (untracked files included).
   - Right-click a stash: Apply, Pop, Rename (git `stash store` + drop; moves it to the top), Copy message, Drop (confirm).
   - Create: "N changed files" header menu -> Stash all changes… / Stash selected files… (pathspecs), Ctrl+Shift+S; optional message; scope radios when only some files are checked. Untracked files are always included (`-u`).
   - Checkout with uncommitted changes always opens Desktop's dialog (Leave my changes on X = stash / Bring my changes to Y). If checkout fails after stashing, the stash is popped back.
   - Conflicts on apply/pop: the error is shown, the working tree is left conflicted (files show "!") for the user to resolve, and a conflicted pop keeps the stash. No dedicated conflict UI (belongs to the in-progress banner work).
   - Known gaps: stashes dropped externally are noticed on the next full refresh (focus/F5/any op), since `git stash drop` of a non-top entry only rewrites the reflog; no auto-restore of the switch-stash when returning to the branch; "Stash selected files" is always enabled in the menu (notice if none checked).
   - Snapshot views: `stash-list`, `stash-all`, `stash-detail`, `stash-detail-untracked`, `stash-menu`, `stash-dialog`, `stash-drop-dialog`, `switch-branch-dialog`. Smoke: `cargo run -p gd-git --example stash_smoke -- /tmp/desktop-rs/stash-smoke [--keep]`.
6. ✅ Merge / squash / rebase into current branch; operation-in-progress banner; conflict resolution; branch context menu.
   - Branch popover footer "Merge into current branch…" (ctrl+shift+M): dialog with Merge | Squash and merge | Rebase modes, filter, ↑/↓/Enter, Desktop-style preview line ("This will merge N commits from X into Y" / "up to date", plus expected conflicts via `git merge-tree`), computed in the background.
   - Right-click a branch: merge, rebase onto, create from, rename, copy name, delete (also offers deleting the remote branch). Conflicted file rows: Use ours / Use theirs / Mark as resolved / Open in default editor.
   - Banner with Continue/Abort for merge, squash, rebase, cherry-pick, revert; commit box prefilled with MERGE_MSG and commits via `merge --continue`-equivalent; Continue disabled while conflicts remain.
   - Snapshot views: `merge-dialog`, `squash-dialog`, `rebase-dialog`, `branch-menu`, `delete-dialog`, `rename-dialog`, `conflict-menu`, and `changes` on a conflicted repo (`cargo run -p gd-git --example merge_smoke -- <dir> --keep-conflict`).
   - Not verified with a real mouse/keyboard. Not done: remote rename, rebase `--autostash`, interactive conflict resolution inside the app, banner for stash-conflicts. Picker ↑/↓ now also capture NavUp/NavDown (the LIST-context bindings never reached a focused input).
7. ✅ Partial (line/hunk) staging in the diff gutter (Changes tab, unified and split).
   - Desktop layout: split view is `[old text | old no · handle · new no | new text]` (numbers in the middle);
     unified is `[handle | check · old no · new no | text]` (one selection cell per line). Click a line number to
     (de)select that line; drag along the numbers to apply the same state to a range. Included lines get a solid blue
     number cell with a check mark.
   - Chunks: each run of changed lines between unchanged lines is one handle (`RepoModel::change_block`). It stays blue
     while any of its lines is included, with one mark on its first row (check = all, dash = some). Hover lights up
     that chunk only; click (de)selects it. Rows outside chunks show a dark strip.
   - Hunk expansion (Changes and History diffs, not stashes): buttons in the hunk header gutter like Desktop. First
     hunk: Expand Up; between hunks: Expand All when the gap is at most 20 lines, else Expand Down + Expand Up; a
     trailing row after the last hunk: Expand Down. 20 lines per click, hunks merge when the gap closes. The expanded
     diff survives watcher reloads while the file is unchanged. Snapshot views `expanded`, `expanded-split`.
   - File checkbox shows Mixed for partially selected files; clicking it selects the whole file again. Header select-all
     clears partial selections.
   - Not selectable: history/stash diffs, "Hide whitespace changes" on (selection is kept for later), truncated or
     non-UTF-8 diffs, conflicted files, while a merge commit is pending. Stash "selected files" still stashes whole files.
   - Verified: `cargo test -p gd-git` (staged_text), `smoke` example (real partial commit). Snapshot views
     `partial[@N]`, `partial-split[@N]`. Not yet clicked with a real mouse.
8. 🟨 Split diff view and whitespace toggle done (gear button in both diff headers: Unified/Split + Hide whitespace changes, persisted in `display` settings file, `-w` via `gd_git::DiffOptions`). Image diffs still open. Split view clips long lines (no horizontal scroll). Snapshot views: `split`, `split-ws`, `split-history`, `hide-ws`, `diff-options-menu`. Split row build: 0.36 ms for an 18k-line diff (cached per loaded diff), so toggling is instant.
9. ⬜ Pull requests list (GitHub API); create PR; CI status.
10. ✅ (branch `rust/history-ops`) History commit actions and push status:

- Unpushed commits get an arrow-up icon with tooltip ("hasn't been pushed to the remote repository yet"); tags show as pills. Refreshed on every full refresh (push/fetch/commit/checkout).
- Right-click menu: Amend, Undo, Reset to commit (soft/mixed dialog), Checkout commit (detached warning), Move commit up/down (reorder), Revert, Create branch from commit, Create/Delete tag, Cherry-pick (branch picker), Copy SHA/tags, View on GitHub. Multi-select (ctrl/cmd-click, shift-click range) offers Cherry-pick N and Squash N (message dialog).
- Amend mode in the commit box ("Amend last commit", Stop amending, force-push warning if pushed).
- Verified by `cargo run -p gd-git --example history_ops -- /tmp/desktop-rs/hops` (bare scratch remote). Snapshot views: `history-menu`, `-menu-head`, `-menu-pushed`, `-multiselect`, `-multimenu`, `-squash`, `-reset`, `-cherry`, `-tag`, `-checkout`, `amend-mode`.
- Skipped: drag-and-drop reorder (menu Move up/down instead), Delete tag restricted to unpushed tags (needs remote tag knowledge; all tags deletable locally), non-adjacent squash (menu item disabled), own conflict UI (uses the generic in-progress banner via `run_op`). Not exercised with a real mouse.

11. ⬜ Tags, branch rename/delete UI, compare branches.
12. ⬜ Settings (theme light/dark, git identity, default editor/shell), "Open in editor/terminal".
13. ✅ Client-side decorations (`app/src/platform/linux/chrome.rs`): toolbar doubles as titlebar (drag empty space, double-click maximize, right-click window menu), caption buttons per desktop `button-layout`, resize strips on free edges, 10px rounded + shadowed frame when floating, square when tiled/maximized. Preview headlessly with `GD_SNAPSHOT_CSD=1 … --snapshot`.
14. ⬜ Packaging (AppImage/deb/rpm, macOS, Windows).
15. 🟡 macOS support (`app/src/platform/macos/`): transparent native titlebar with traffic lights, cmd shortcuts, Application Support config dir. **Written but never compiled or run** (no Mac or `aarch64-apple-darwin` target here); first thing to do on a Mac is `cargo build` and check traffic-light position / toolbar inset.

## Test repo recipe

```sh
mkdir -p /tmp/desktop-rs && cd /tmp/desktop-rs && rm -rf testrepo && mkdir testrepo && cd testrepo && git init -q -b main
for i in 1 2 3 4 5; do echo "line $i" >> file$i.txt; mkdir -p src/mod$i
  printf 'fn main() {\n    println!("hello %s");\n}\n' $i > src/mod$i/main.rs
  git add -A; git commit -qm "Add module $i with some changes" -m "Body text for commit $i"; done
echo changed >> file1.txt; sed -i 's/hello/goodbye/' src/mod2/main.rs; echo new > untracked.md; git rm -q file5.txt
```
