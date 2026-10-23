//! Branch-ops state: the right-click menu and the rename / delete /
//! create-from dialogs ([`BranchUi`], a plain `Workspace` field).

use gd_git::Branch;
use gpui::{AppContext, Context, Entity, Pixels, Point, Subscription};
use gpui_elements::editable_text::{EditableTextState, StringStorage, TextChanged};

use crate::workspace::Workspace;

#[derive(Clone)]
pub enum BranchDialog {
    Rename { old: String },
    /// `remote` is the upstream (`origin/x`) that can be deleted along with it.
    Delete { branch: Branch, remote: Option<String>, also_remote: bool },
    CreateFrom { base: String },
}

pub struct BranchMenu {
    pub position: Point<Pixels>,
    pub branch: Branch,
    pub selected: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BranchAction {
    Merge,
    Rebase,
    CreateFrom,
    Rename,
    CopyName,
    Delete,
}

/// A branch on a remote, to delete on the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteBranch {
    pub remote: String,
    pub branch: String,
}

impl RemoteBranch {
    /// Parse `origin/feature/x` (remote name = first segment).
    pub fn parse(full: &str) -> Option<Self> {
        let (remote, branch) = full.split_once('/')?;
        Some(Self { remote: remote.to_string(), branch: branch.to_string() })
    }
}

/// Everything the branch ops add to `Workspace`. A plain struct (not an
/// `Entity`): Esc/Enter/arrow routing in workspace.rs/menus.rs reads `menu`/`dialog`.
pub struct BranchUi {
    pub menu: Option<BranchMenu>,
    pub(super) menu_gen: usize,
    pub dialog: Option<BranchDialog>,
    /// Name typed into the rename / create-from dialogs.
    pub input: Entity<EditableTextState>,
    _subs: Vec<Subscription>,
}

impl BranchUi {
    pub fn new(cx: &mut Context<Workspace>) -> Self {
        let input = cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        let subs = vec![cx.subscribe(&input, |_: &mut Workspace, _, _: &TextChanged, cx| cx.notify())];
        Self { menu: None, menu_gen: 0, dialog: None, input, _subs: subs }
    }

    /// Whether the root should carry the "Menu" key context (↑/↓/Enter/Esc
    /// reach the workspace instead of an input).
    pub fn wants_menu_context(&self) -> bool {
        self.menu.is_some() || matches!(self.dialog, Some(BranchDialog::Delete { .. }))
    }
}
