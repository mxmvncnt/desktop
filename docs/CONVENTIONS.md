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
  Esc/Enter routing in workspace/actions.rs and menus reads `stash_ui.dialog`; only
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
  `MenuRow::{Item(RowItem), Separator}`); `features/menus` renders them.
- Dialog variants are built by one small method per variant on the
  `RenderOnce` struct, not one giant match.
- Feature-owned menus reuse `MenuRow<A>` with their own action enum
  (`features/history`: `MenuRow<HistoryAction>`); rows are built in one small fn
  per group (`commit_menu_rows.rs`).
- Identical rows across features live in `shared/` (`file_change_row.rs`, with an
  id namespace and an `on_select` callback prop).
- Replace `(bool, bool)` click flags with an enum (`ClickKind`).
- Two related features can share one overlay area without sharing state:
  `merge` (`merge_ui`) and `branches` (`branch_ui`) each own their menu/dialog;
  workspace/actions.rs / menus route Esc/Enter/arrows with `a() || b()`. Typed structs
  replace tuples (`branches::RemoteBranch`, not `(String, String)`).

## Feature folder example: diff (hot rows)

`crates/app/src/features/diff/` follows the same layout (`state.rs` = `DiffView`
and gutter pointer state, `handlers.rs` = `impl Workspace` setters,
`render_diff.rs` = the entry point, pure logic in `split_diff.rs`,
`highlight.rs`, `line_selection.rs`). Decisions:

- **Allowed exception: hot rows stay plain fns/methods.** Virtualized rows are
  rebuilt for every visible row every frame, so `UnifiedRows::row`,
  `SplitRows::row`, `number_cell` and `RowSelect::handle_cell` are not
  `RenderOnce` structs (no per-row props struct, no extra boxing). Per-list and
  per-frame pieces are components (`UnifiedList`, `SplitList`, `DiffFrame`,
  `DiffOptionsMenu`, `DiffOptionsButton`, `WhitespaceNote`, `CheckOptionRow`,
  and `ExpandButton`, which only appears on hunk headers).
- Row renderers borrow their shared inputs in one small struct
  (`UnifiedRows { diff, hl, sel, exp, .. }`) instead of 8 positional args.
- Keep the perf tricks when touching this folder: `Arc::ptr_eq` caches
  (`split_for`, `wrap_list_for`), `restrict_scroll_to_axis` and the manual
  h-scroll in `rows_list`/`text_flow`, and no `h_full` on gutter cells.
- Typed replacements: `ExpandTarget` (not `commit: bool`), `BlockRow::{First,
  Rest}`, `HoveredHandle`/`GutterDrag` (not tuples).

## Feature folders: changes, pickers, settings, menus, welcome

- `features/changes`: `Sidebar` (tabs + `ChangesPanel` or `CommitList`),
  `ChangesPanel` composes `ChangesHeader`, `ChangesList`, `StashPanel`,
  `UndoStrip`, `CommitBox` (+ `AmendNotice`). `Sidebar::new(ws, repo, cx)` and
  `ChangesPanel::new` read the workspace once and hand typed props down;
  derived values live in `state.rs` (`SelectionSummary`, `CommitButton`, instead
  of a dozen locals). The main pane (`render_changes_main`) stays an
  `impl Workspace` method (diff renderer needs `&mut Workspace`). Hot row:
  `change_row` is a plain fn. Changes-specific `RepoModel` ops (include/exclude,
  commit, ignore, discard, undo) are in `ops.rs`; `select_change` stays in
  `repo/change_diff.rs` because it drives the private diff loader.
- `features/pickers`: `PickerShell` owns filter input, empty state and
  Enter/up/down capture; `RepoPicker`/`BranchPicker` pass an `on_enter` callback.
  `Workspace::repo_picker()/branch_picker()/new_branch_form()/clone_form()`
  (handlers.rs) clamp the cursor and build the components; popover state stays
  on `Workspace`. Hot rows `repo_row`/`branch_row` are plain fns.
- `features/settings`: also owns the persisted `Settings` model (`model.rs`).
  `SettingsModal::open(ws, cx)` -> `Option`, one `RenderOnce` per page.
- `features/menus`: the generic context menu (`ContextMenuView`), discard
  dialog (`DiscardDialogView`), `MenuAction`/`MenuTarget`, and the cross-feature
  arrow/Enter routing (`key_routing.rs`).
- `shared/`: `file_status_color`, `split_path`, `branch_name`
  (`sanitize_branch_name`), `suggestion_card` (`SuggestionCard`).
- Closure-heavy `RenderOnce` components call back through `Entity<Workspace>`
  (`ws.update(cx, |this, cx| ..)`), never `cx.listener`.
- Return types that borrow `&self` need `+ use<>` (edition 2024).
