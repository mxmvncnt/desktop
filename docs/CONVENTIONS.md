# Code conventions (app crate)

The goal is code a human can read in small pieces, organized like a Vue app.

## Feature folders

UI code lives in `crates/app/src/features/<feature>/`, one folder per feature
(stashes, history, merge, changes, diff, pickers, settings, welcome, ...).

```
features/stashes/
  mod.rs            // only `mod` lines + `pub use` of the public surface
  stashes_panel.rs  // container component for the feature
  stash_row.rs      // small component, one per file
  stash_dialog.rs
  state.rs          // state structs + enums of the feature
  ops.rs            // RepoModel ops / background work of the feature
```

- Use `pub(super)` / `pub(crate)` to keep internals private. The rest of the app
  only sees what `mod.rs` re-exports.
- Shared, feature-independent bits go in `crates/app/src/shared/` (or `gd-ui` if
  they're pure design-system components). No `utils.rs`/`helpers.rs` grab bags:
  name a file after what it does (`relative_time.rs`).

## Components (Vue SFC ≈ one file)

| Vue | GPUI |
|---|---|
| stateless SFC with props | `#[derive(IntoElement)] struct X { .. }` + `impl RenderOnce` |
| SFC with `data()` | `Entity<X>` + `impl Render` |
| emit | `EventEmitter<E>` + `cx.subscribe`, or a callback field |
| store | `Entity<RepoModel>` or a `cx.global` |
| template | the `render()` fn |

- Default to `RenderOnce`. Only make an `Entity` when it owns state that
  outlives a frame (text input, scroll handle, open dialog, selection).
- **One `Render`/`RenderOnce` impl per file.** Small private
  `fn header(..) -> impl IntoElement` pieces are fine; once a piece needs its own
  props, it gets its own file.

## File layout: template first

1. The struct (props/state), short.
2. `impl Render` / `impl RenderOnce` (the "template").
3. `impl X { .. }`: handlers and logic.
4. Private helper fns.

## Functions and types

- Small functions (aim < 40 lines; render fns compose sub-pieces).
- Typed enums/structs instead of tuples, `bool` flags, or strings
  (e.g. `enum MenuRow { Item(MenuItem), Separator }`, not
  `Option<(MenuAction, &str, Icon, bool)>`).

## Dependencies

Prefer a well-maintained crate over hand-rolled platform/utility code
(config dirs, debouncing, serialization, time formatting, opening URLs, trash...).
Hand-roll only when a crate is too restrictive, and write down why.
Exception: git writes and network ops keep shelling out to the `git` CLI
(hooks, credential helpers, LFS, SSH behave exactly like the CLI).
