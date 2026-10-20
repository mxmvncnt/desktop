# Architecture

## Workspace layout

```
desktop-rs/
├── Cargo.toml          workspace; GPUI-CE pinned by git rev
├── env.sh              source before cargo: CARGO_HOME/TMPDIR inside the project
├── crates/
│   ├── git/   (gd-git)  UI-agnostic git layer, shells out to `git`
│   ├── ui/    (gd-ui)   zeron-style design system: theme, motion, components, assets
│   └── app/   (github-desktop)  the binary: models + views
├── docs/
└── .ref/ .cargo-home/ .tmp/ target/   (git-ignored; see below)
```

`.ref/` holds read-only reference clones: `.ref/zeron` (the design source) and
`.ref/gpui-ce` (the framework source at our pinned rev). Read those instead of
guessing APIs. To recreate them:
`git clone --depth 1 https://github.com/zeronsh/zeron .ref/zeron` and the same for `gpui-ce/gpui-ce`.

## Git layer (`crates/git`)

- Like Desktop's `dugite`, it **shells out to the system `git`** (`exec.rs`).
  Hooks, credential helpers, LFS, config, and SSH all behave as they do on the CLI.
  It sets `LC_ALL=C`, `GIT_OPTIONAL_LOCKS=0`, and `core.quotepath=false`.
  Network ops set `GIT_TERMINAL_PROMPT=0`, so credentials must come from a helper or ssh-agent.
  (An auth UI is still to do.)
- It only parses machine formats: `status --porcelain=v2 -z`,
  `log -z` with `%x1f` separators, `for-each-ref` with `%00`, `show --name-status -z`.
- **All functions block.** Call them only from a background executor.
- Commit model (`Repository::commit`): reset the index to HEAD, `add -A` the
  checked paths, then `commit -F -`. This matches Desktop's checkbox semantics.
  Partial-line commits aren't supported yet.
- Diffs are parsed into a flat `Vec<DiffLine>` (hunk headers included) so the UI
  can index row `i` in O(1) from a `uniform_list`. Parsing is capped at 50k lines (`truncated`).
- `examples/smoke.rs` exercises the write paths against a scratch repo:
  `cargo run -p gd-git --example smoke -- $PWD/.tmp/smoke`.

## Threading / state (`crates/app`)

- `Workspace` (`workspace.rs`) is the root view. It owns settings, popover state,
  text-input entities, scroll handles, and an `Option<Entity<RepoModel>>`.
- `RepoModel` (`repo.rs`) owns all repository state: status, selection, diff,
  history pages, commit details, branches, and in-flight ops.
  Every git call goes through `cx.background_spawn`, and results are applied with `this.update`.
- **Stale results are dropped** with generation counters (`change_diff_gen`,
  `commit_gen`). Clicking quickly through files never shows the wrong diff.
- `RepoModel::run(op, on_ok)` is the generic write-op helper. It runs off-thread,
  sets `error`, then refreshes status and branches.
- Refresh triggers: window activation (like Desktop), after every op, F5.
  There is no file watcher yet (see PROGRESS).
- Views live in `views/*.rs` as `impl Workspace` blocks. Rows in lists hold
  `Entity<RepoModel>` clones and call `update` in click handlers.

### Performance rules (primary goal)

1. Never run git on the UI thread.
2. Any list that can be long must be a `uniform_list`: file lists, history, diffs, pickers.
3. History is paged (`HISTORY_PAGE` = 200). More pages load as the list nears its end.
4. Animations are one-shot. Don't use `repeat()` animations for idle UI. The only
   repeating one is the toolbar busy pulse, and it exists only during network ops.
   (zeron's `motion.rs` notes that a repeating animation pins the window at 120 Hz.)
5. The dev profile builds deps at `opt-level=3` so debug runs are representative.
6. Release profile: thin LTO, `codegen-units=1`.

Known perf TODOs: split `Workspace` into child entities so typing in the commit
box doesn't re-render the diff (it's virtualized, so it's cheap but not free);
add syntax highlighting with a cache; add a file watcher so focus refresh isn't the only trigger.

## UI kit (`crates/ui`)

- `theme.rs`: zeron's dark tokens, verbatim (oklch → sRGB helper included).
  Use `Theme::of(cx)`, `ink(a)` for translucent white washes, and `hairline(a)` for lines.
- `motion.rs`: the zeron motion catalog (`fade_in` 500ms expo-out with 4px rise,
  `fade_quick`, `menu_in`, `dialog_in`, plus an exact `CubicBezier`).
- `components.rs`: `pressable` (row/ghost base with transitioned hover wash), `Button`
  (Primary, Secondary, Ghost, Danger), `Checkbox`, `Tabs` (segmented), `ToolbarButton`
  (Desktop's toolbar dropdowns), `badge`, `overlay_card`, `divider`/`vdivider`, `icon`.
- `assets.rs`: embedded Solar icons and Geist fonts. To add an icon, copy the
  SVG from `.ref/zeron/crates/ui/assets/icons/` into `crates/ui/assets/icons/`
  and add a line to the `icons!` macro.

## GPUI-CE gotchas (learned the hard way)

- **`AssetSource::list("")` must enumerate every asset.** GPUI-CE builds its
  `AssetRegistry` from it once at startup. If an asset isn't listed, `svg()`
  silently draws nothing.
- Colors are `palette` types. `.opacity()` needs `use gpui::ColorExt`. Build
  colors with `gpui::rgb_to_hsla(gpui::Rgba::new(r,g,b,a))`, not struct literals.
- `with_animation` needs `use gpui::AnimationExt`.
- Style transitions: `.transitions(|t| t.bg(millis(120)).text_color(..))` plus
  `.hover_style(..)` / `.active_style(..)`. Transitionable properties include
  `bg`, `border_color`, `text_color`, sizes, insets, and ring.
  `.hover(..)` and `.group_hover(..)` are deprecated aliases of the `_style` forms.
- `.flex_shrink(f32)` takes an argument.
- `uniform_list(id, n, |range, window, cx| ..)` has no entity parameter. Capture an `Entity` clone.
- Edition 2024: `-> impl IntoElement` on `&mut self` methods captures the borrow.
  Use `-> impl IntoElement + use<>` (only when no APIT args), or return `AnyElement`.
- Text inputs: `gpui_elements::editable_text::{text_input, text_area, EditableTextState}`.
  The state is an entity (`as_str()`, `emplace()`). Subscribe to `TextChanged` to re-render.
  Bind `default_bindings()` for `DEFAULT_INPUT_CONTEXT`. Catch Enter with
  `.capture_action(|_: &editable_text::actions::Enter, ..|)` on a wrapper.
- Headless snapshots run on a fake dispatcher clock, but **animations use wall-clock time**.
  `snapshot.rs` sleeps for real and refreshes before capturing. Without that,
  fades are captured at opacity 0.
- `import`/`grim` aren't available on the dev machine. Use `--snapshot` to see the UI.

## GPUI-CE pin

`Cargo.toml` pins `rev = 9a82fba4…` (2026-10-18). To bump it, change the rev
in all four entries, run `cargo update -p gpui-ce`, rebuild, and re-check the
gotchas above.
