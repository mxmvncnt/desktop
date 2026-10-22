//! Actions and default keybindings (mirrors GitHub Desktop's shortcuts;
//! `secondary` = ctrl on Linux, cmd on macOS, see `platform::SECONDARY`).

use gpui::{App, KeyBinding, Menu, MenuItem, OsAction, SystemMenuType, actions};

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
        OpenSettings,
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
        KeyBinding::new(&format!("{M}-,"), OpenSettings, ws),
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
    set_menus(cx);
}

/// Native main menu bar (shown on macOS; a no-op elsewhere).
fn set_menus(cx: &mut App) {
    cx.set_menus([
        Menu::new("GitHub Desktop").items([
            MenuItem::action("Settings…", OpenSettings),
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Quit GitHub Desktop", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("Add Local Repository…", AddRepository),
        ]),
        Menu::new("Edit").items([
            MenuItem::os_action("Undo", gpui_elements::editable_text::actions::Undo, OsAction::Undo),
            MenuItem::os_action("Redo", gpui_elements::editable_text::actions::Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action("Cut", gpui_elements::editable_text::actions::Cut, OsAction::Cut),
            MenuItem::os_action("Copy", gpui_elements::editable_text::actions::Copy, OsAction::Copy),
            MenuItem::os_action("Paste", gpui_elements::editable_text::actions::Paste, OsAction::Paste),
            MenuItem::os_action("Select All", gpui_elements::editable_text::actions::SelectAll, OsAction::SelectAll),
        ]),
        Menu::new("View").items([
            MenuItem::action("Changes", ShowChanges),
            MenuItem::action("History", ShowHistory),
            MenuItem::separator(),
            MenuItem::action("Repository List", ToggleRepositoryList),
            MenuItem::action("Branch List", ToggleBranchList),
            MenuItem::separator(),
            MenuItem::action("Refresh", Refresh),
        ]),
        Menu::new("Repository").items([
            MenuItem::action("Fetch", Fetch),
            MenuItem::action("Pull", Pull),
            MenuItem::action("Push", Push),
            MenuItem::separator(),
            MenuItem::action("Stash Changes…", StashChanges),
        ]),
        Menu::new("Branch").items([
            MenuItem::action("New Branch…", NewBranch),
            MenuItem::action("Merge into Current Branch…", MergeIntoCurrent),
            MenuItem::separator(),
            MenuItem::action("Commit", Commit),
        ]),
        Menu::new("Window").items([]),
    ]);
}
