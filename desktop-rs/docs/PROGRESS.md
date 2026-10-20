# Progress

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
  `desktop-rs/.worktrees/<name>` (branch `rust/<name>`), then merge with `--no-ff`
  and resolve the predictable conflicts (`repo.rs` fields, `snapshot.rs` view list, docs).

Branch: `rust-rewrite`. Each entry: ✅ done · 🟡 partial · ⬜ todo.

## POC (session 1, 2026-10-20)

### Foundation
- ✅ Cargo workspace (`gd-git`, `gd-ui`, `github-desktop`), GPUI-CE pinned.
- ✅ Project-local cargo, tmp, and config (`env.sh`).
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
- Notes: headless test shows ~1s from last write to updated status (300ms debounce + dispatcher wake-up latency); real sleeps + `run_until_parked` polling are required. Debug log: `RUST_LOG=github_desktop=debug` prints one line per refresh, `trace` prints fs events.

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
   - Snapshot views: `stash-list`, `stash-all`, `stash-detail`, `stash-detail-untracked`, `stash-menu`, `stash-dialog`, `stash-drop-dialog`, `switch-branch-dialog`. Smoke: `cargo run -p gd-git --example stash_smoke -- $PWD/.tmp/stash-smoke [--keep]`.
6. ⬜ Merge / rebase / squash into current branch; conflict resolution UI.
7. ⬜ Partial (line/hunk) staging in the diff gutter.
8. 🟨 Split diff view and whitespace toggle done (gear button in both diff headers: Unified/Split + Hide whitespace changes, persisted in `display` settings file, `-w` via `gd_git::DiffOptions`). Image diffs still open. Split view clips long lines (no horizontal scroll). Snapshot views: `split`, `split-ws`, `split-history`, `hide-ws`, `diff-options-menu`. Split row build: 0.36 ms for an 18k-line diff (cached per loaded diff), so toggling is instant.
9. ⬜ Pull requests list (GitHub API); create PR; CI status.
10. ⬜ Cherry-pick, revert, amend, reorder, and squash commits from History.
11. ⬜ Tags, branch rename/delete UI, compare branches.
12. ⬜ Settings (theme light/dark, git identity, default editor/shell), "Open in editor/terminal".
13. ✅ Client-side decorations (`app/src/chrome.rs`): toolbar doubles as titlebar (drag empty space, double-click maximize, right-click window menu), caption buttons per desktop `button-layout`, resize strips on free edges, 10px rounded + shadowed frame when floating, square when tiled/maximized. Preview headlessly with `GD_SNAPSHOT_CSD=1 … --snapshot`.
14. ⬜ Packaging (AppImage/deb/rpm, macOS, Windows).

## Test repo recipe

```sh
cd desktop-rs/.tmp && rm -rf testrepo && mkdir testrepo && cd testrepo && git init -q -b main
for i in 1 2 3 4 5; do echo "line $i" >> file$i.txt; mkdir -p src/mod$i
  printf 'fn main() {\n    println!("hello %s");\n}\n' $i > src/mod$i/main.rs
  git add -A; git commit -qm "Add module $i with some changes" -m "Body text for commit $i"; done
echo changed >> file1.txt; sed -i 's/hello/goodbye/' src/mod2/main.rs; echo new > untracked.md; git rm -q file5.txt
```
