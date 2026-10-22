# CLAUDE.md

This branch hosts a **Rust + GPUI-CE rewrite** of GitHub Desktop; it is the whole repository.
The original Electron app is only in git history (`development` branch) and is
the behavioral reference, e.g. `git show development:app/src/...`.

Start here:
1. `docs/AGENTS.md`: hard rules. **Never touch files outside this
   project directory.**
2. `docs/PROGRESS.md`: current state and next steps.
3. `docs/ARCHITECTURE.md`: code map, threading, GPUI-CE gotchas.
4. `docs/DESIGN.md`: zeron visual language.
