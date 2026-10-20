//! Actions and default keybindings (mirrors GitHub Desktop's shortcuts on
//! Linux/Windows; `secondary` = ctrl on Linux, cmd on macOS).

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
    cx.bind_keys([
        KeyBinding::new("ctrl-1", ShowChanges, Some(WORKSPACE)),
        KeyBinding::new("ctrl-2", ShowHistory, Some(WORKSPACE)),
        KeyBinding::new("ctrl-t", ToggleRepositoryList, Some(WORKSPACE)),
        KeyBinding::new("ctrl-b", ToggleBranchList, Some(WORKSPACE)),
        KeyBinding::new("ctrl-shift-n", NewBranch, Some(WORKSPACE)),
        KeyBinding::new("ctrl-shift-m", MergeIntoCurrent, Some(WORKSPACE)),
        KeyBinding::new("ctrl-o", AddRepository, Some(WORKSPACE)),
        KeyBinding::new("ctrl-shift-t", Fetch, Some(WORKSPACE)),
        KeyBinding::new("ctrl-shift-p", Pull, Some(WORKSPACE)),
        KeyBinding::new("ctrl-p", Push, Some(WORKSPACE)),
        KeyBinding::new("f5", Refresh, Some(WORKSPACE)),
        KeyBinding::new("ctrl-enter", Commit, Some(WORKSPACE)),
        KeyBinding::new("escape", Dismiss, Some(WORKSPACE)),
        KeyBinding::new("ctrl-q", Quit, None),
        KeyBinding::new("down", SelectNext, Some(LIST)),
        KeyBinding::new("up", SelectPrevious, Some(LIST)),
        KeyBinding::new("down", MenuNext, Some(MENU)),
        KeyBinding::new("up", MenuPrevious, Some(MENU)),
        KeyBinding::new("enter", MenuConfirm, Some(MENU)),
    ]);
    cx.on_action(|_: &Quit, cx| cx.quit());
}
