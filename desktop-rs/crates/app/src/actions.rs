//! Actions and default keybindings (mirrors GitHub Desktop's shortcuts;
//! `secondary` = ctrl on Linux, cmd on macOS, see `platform::SECONDARY`).

use gpui::{App, KeyBinding, actions};

actions!(
    desktop,
    [
        ShowChanges,
        ShowHistory,
        ToggleRepositoryList,
        ToggleBranchList,
        NewBranch,
        MergeIntoCurrent,
        AddRepository,
        Fetch,
        Pull,
        Push,
        Refresh,
        Commit,
        StashChanges,
        SelectNext,
        SelectPrevious,
        Dismiss,
        MenuNext,
        MenuPrevious,
        MenuConfirm,
        Quit,
    ]
);

/// Key context set on the workspace root.
pub const WORKSPACE: &str = "Workspace";
/// Key context set on keyboard-navigable lists.
/// Key context added to the workspace root while a menu or dialog is open.
pub const MENU: &str = "Menu";
/// Key context set on keyboard-navigable lists.
pub const LIST: &str = "List";

pub fn bind(cx: &mut App) {
    use crate::platform::{QUIT_KEY, REFRESH_KEY, SECONDARY as M};
    let ws = Some(WORKSPACE);
    cx.bind_keys([
        KeyBinding::new(&format!("{M}-1"), ShowChanges, ws),
        KeyBinding::new(&format!("{M}-2"), ShowHistory, ws),
        KeyBinding::new(&format!("{M}-t"), ToggleRepositoryList, ws),
        KeyBinding::new(&format!("{M}-b"), ToggleBranchList, ws),
        KeyBinding::new(&format!("{M}-shift-n"), NewBranch, ws),
        KeyBinding::new(&format!("{M}-shift-m"), MergeIntoCurrent, ws),
        KeyBinding::new(&format!("{M}-o"), AddRepository, ws),
        KeyBinding::new(&format!("{M}-shift-t"), Fetch, ws),
        KeyBinding::new(&format!("{M}-shift-p"), Pull, ws),
        KeyBinding::new(&format!("{M}-p"), Push, ws),
        KeyBinding::new(REFRESH_KEY, Refresh, ws),
        KeyBinding::new(&format!("{M}-enter"), Commit, ws),
        KeyBinding::new(&format!("{M}-shift-s"), StashChanges, ws),
        KeyBinding::new("escape", Dismiss, ws),
        KeyBinding::new(QUIT_KEY, Quit, None),
        KeyBinding::new("down", SelectNext, Some(LIST)),
        KeyBinding::new("up", SelectPrevious, Some(LIST)),
        KeyBinding::new("down", MenuNext, Some(MENU)),
        KeyBinding::new("up", MenuPrevious, Some(MENU)),
        KeyBinding::new("enter", MenuConfirm, Some(MENU)),
    ]);
    cx.on_action(|_: &Quit, cx| cx.quit());
}
