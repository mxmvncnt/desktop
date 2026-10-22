# Code conventions (app crate)

The goal is code a human can read in small pieces, organized like a Vue app.

## Feature folders

UI code lives in `crates/app/src/features/<feature>/`, one folder per feature
(stashes, history, merge, changes, diff, pickers, settings, welcome, ...).

```
features/stashes/
  mod.rs            // only `mod` lines + `pub use` of the public surface
  stash_panel.rs    // container component for the feature
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

## Feature folder example: stashes

`crates/app/src/features/stashes/` is the reference. Copy these decisions:

- `mod.rs` only has `mod` lines and `pub use` (panel, dialog view, menu rows,
  state types). Everything else is private to the folder.
- `state.rs`: `StashState` (model state, a field of `RepoModel`), `StashDialog`
  enum, `StashUi` (dialog + input, still a plain `Workspace` field because
  Esc/Enter routing in workspace.rs/menus.rs reads `stash_ui.dialog`; only
  promote it to an `Entity` if that routing is reworked).
- `ops.rs`: `impl RepoModel` (background git work, generation counter).
- `handlers.rs`: `impl Workspace` handlers (open/close/confirm dialogs, menu
  actions). `pub(super)` for what sibling components call, `pub` only for what
  the rest of the app calls.
- Components are `#[derive(IntoElement)]` + `RenderOnce`, one per file
  (`stash_panel`, `stash_panel_header`, `stash_list`, `stash_row`,
  `stash_header`, `stash_file_list`, `stash_file_row`, `stash_dialog`,
  `option_row`), with a `new(..)` constructor and typed props.
- Props: data they show (cloned, e.g. `Stash`) plus `Entity<RepoModel>` for
  model calls and `Entity<Workspace>` (`cx.entity()` at the call site) for UI
  actions. Handlers call back with
  `ws.update(cx, |this, cx| this.open_stash_dialog(..))` or
  `repo.update(cx, |r, cx| r.stash_pop(sha, cx))`; no `cx.listener` inside
  `RenderOnce`. Callback props are `Box<dyn Fn(&mut Window, &mut App)>`
  (`OptionRow`).
- Pieces that need `&mut Workspace` (the diff renderer owns scroll state) stay
  an `impl Workspace` method (`stash_detail.rs`) that composes the stateless
  components around it.
- Panels that may be hidden expose `Component::new(..) -> Option<Self>`.
- Context menus return `Vec<MenuRow>` (`shared/menu_row.rs`:
  `MenuRow::{Item(RowItem), Separator}`); `views/menus.rs` renders them.
- Dialog variants are built by one small method per variant on the
  `RenderOnce` struct, not one giant match.
