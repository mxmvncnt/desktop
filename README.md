# desktop-rs — Rust + GPUI-CE rewrite

A native rewrite of [GitHub Desktop](https://github.com/desktop/desktop) (the original Electron app lives in the `development` branch) in Rust, on
[GPUI Community Edition](https://github.com/gpui-ce/gpui-ce), with the look and
motion of [zeron](https://github.com/zeronsh/zeron).

Status: **POC**. Docs:

- [docs/PROGRESS.md](docs/PROGRESS.md): what works, what's next (the feature ladder).
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md): crates, threading, the git layer, the UI kit, GPUI-CE gotchas.
- [docs/AGENTS.md](docs/AGENTS.md): rules for AI sessions and subagents. **Read this first if you're an agent.**
- [docs/DESIGN.md](docs/DESIGN.md): the zeron visual language and how it maps to Desktop's layout.

## Build and run

```sh
cargo run --release           # or: cargo run -- /path/to/repo
```

Linux needs the usual GPUI deps (xkbcommon, wayland, x11-xcb, fontconfig, vulkan driver).
The system `git` binary must be on `PATH`.

The app stores its repository list in `$GD_CONFIG_DIR` (if set), otherwise in
`~/.config/desktop-rs/`.

### Keyboard shortcuts

| Keys            | Action                                    |
| --------------- | ----------------------------------------- |
| Ctrl+1 / Ctrl+2 | Changes / History tab                     |
| Ctrl+T          | Repository list                           |
| Ctrl+B          | Branch list                               |
| Ctrl+Shift+N    | New branch                                |
| Ctrl+O          | Add existing repository                   |
| Ctrl+Shift+T    | Fetch                                     |
| Ctrl+Shift+P    | Pull                                      |
| Ctrl+P          | Push / publish                            |
| Ctrl+Enter      | Commit                                    |
| F5              | Refresh                                   |
| ↑ / ↓           | Move through file, commit or picker lists |
| Esc             | Close popover                             |
| Ctrl+Q          | Quit                                      |

### Headless screenshots

```sh
GD_CONFIG_DIR=/tmp/desktop-rs/config cargo run --features snapshot -- \
    --snapshot /tmp/desktop-rs/shot.png /path/to/repo [changes|history|repos|branches|new-branch|clone|welcome|icons]
```

This renders the real UI offscreen with wgpu and writes a PNG. It's how agents
check visuals without a display.
