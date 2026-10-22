# Rules for AI sessions and subagents

## Hard rules

1. **Never read, create, or modify files outside `/home/maxime/Git/github-desktop`.**
   - Always `source env.sh` before cargo. It puts `CARGO_HOME`,
     `TMPDIR`, `CARGO_TARGET_DIR`, and `GD_CONFIG_DIR` inside the project.
   - Scratch repos, screenshots, and logs go in `.tmp/` (git-ignored).
   - Don't run the app without `GD_CONFIG_DIR` set (env.sh sets it). Otherwise
     it writes to `~/.config`.
   - Don't `git clone` or `curl` into `/tmp` or `~`. Reference repos live in `.ref/`.
   - Don't touch the user's global git config, `~/.cargo`, or other repos.
2. Only touch files inside this repository. The original Electron app (removed
   from this tree) lives in git history, e.g. `git show development:app/src/...`;
   read it for behavior, don't try to restore it.
3. Work on your own branch and commit your own work (see Workflow). Never
   push or force-push, and never rewrite `rust-rewrite` history.
4. Performance is a primary goal. Follow the rules in ARCHITECTURE.md
   §Performance: no git on the UI thread, virtualize lists, no idle repeating animations.

## Workflow for parallel subagents

- Integration branch: `rust-rewrite`.
- Each subagent gets its own git worktree inside the project:
  ```sh
  cd /home/maxime/Git/github-desktop
  git worktree add .worktrees/<name> -b rust/<name> rust-rewrite
  # Don't copy target/ from another checkout: cargo fingerprints include the
  # absolute path, so it rebuilds everything anyway (~4 min cold, ~7.5 GB).
  cd .worktrees/<name> && source env.sh
  ```
- Commit in small, logical commits with clear messages on `rust/<name>`.
- Before your final commit: `cargo build` and `cargo build --features snapshot`
  must be warning-free, and you must check your UI with a `--snapshot` PNG.
- The coordinator (main session) merges `rust/<name>` into `rust-rewrite` and
  removes the worktree.

## Verifying UI without a display

```sh
cargo run --features snapshot -- --snapshot $PWD/.tmp/x.png <repo> <view>
```
Then look at the PNG. Make test repos under `.tmp/` (see PROGRESS.md for a
script). The snapshot can only open a view, not click. For interaction states,
add a view name to `crates/app/src/snapshot.rs` that sets up the state directly.

## Documentation duty

Every change that adds a feature, a gotcha, or a decision updates:
- `docs/PROGRESS.md`: tick the feature and add notes.
- `docs/ARCHITECTURE.md`: for new modules, threading, or GPUI-CE gotchas.
Future sessions rely only on these docs, so write down what you learned.
