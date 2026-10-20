# Progress

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
- ✅ discard changes (tracked and untracked); undo last commit (soft reset). Both exist in the model, but there's no UI yet.
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

## Next rungs of the ladder (suggested order)

1. ⬜ Changes context menu: discard (with confirm dialog), copy path, reveal, ignore. Undo-last-commit UI.
2. ⬜ File-system watcher (debounced) for live status updates.
3. ⬜ Syntax highlighting in diffs (background, cached).
4. ⬜ Auth: GIT_ASKPASS helper → in-app credential dialog; GitHub sign-in (OAuth device flow).
5. ⬜ Stash (Desktop's "stashed changes"), switch-branch-with-changes dialog.
6. ⬜ Merge / rebase / squash into current branch; conflict resolution UI.
7. ⬜ Partial (line/hunk) staging in the diff gutter.
8. ⬜ Split diff view; image diffs; whitespace toggle.
9. ⬜ Pull requests list (GitHub API); create PR; CI status.
10. ⬜ Cherry-pick, revert, amend, reorder, and squash commits from History.
11. ⬜ Tags, branch rename/delete UI, compare branches.
12. ⬜ Settings (theme light/dark, git identity, default editor/shell), "Open in editor/terminal".
13. ⬜ Client-side decorations (custom titlebar) like zeron.
14. ⬜ Packaging (AppImage/deb/rpm, macOS, Windows).

## Test repo recipe

```sh
cd desktop-rs/.tmp && rm -rf testrepo && mkdir testrepo && cd testrepo && git init -q -b main
for i in 1 2 3 4 5; do echo "line $i" >> file$i.txt; mkdir -p src/mod$i
  printf 'fn main() {\n    println!("hello %s");\n}\n' $i > src/mod$i/main.rs
  git add -A; git commit -qm "Add module $i with some changes" -m "Body text for commit $i"; done
echo changed >> file1.txt; sed -i 's/hello/goodbye/' src/mod2/main.rs; echo new > untracked.md; git rm -q file5.txt
```
