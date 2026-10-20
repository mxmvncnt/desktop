# CLAUDE.md

This branch hosts a **Rust + GPUI-CE rewrite** of GitHub Desktop in `desktop-rs/`.
The Electron app in `app/` is the behavioral reference. Don't modify it.

Start here:
1. `desktop-rs/docs/AGENTS.md`: hard rules. **Never touch files outside this
   project directory.** Always `source desktop-rs/env.sh` before cargo.
2. `desktop-rs/docs/PROGRESS.md`: current state and next steps.
3. `desktop-rs/docs/ARCHITECTURE.md`: code map, threading, GPUI-CE gotchas.
4. `desktop-rs/docs/DESIGN.md`: zeron visual language.
