use gd_git::FileChange;
use gpui::{Pixels, Point};

/// What was right-clicked.
#[derive(Clone)]
pub enum MenuTarget {
    /// A row of the changes list (resolved to the file at open time).
    File(FileChange),
    /// The "N changed files" header.
    Header,
    /// A row of the stash list.
    Stash(gd_git::Stash),
}

pub struct OpenMenu {
    pub position: Point<Pixels>,
    pub target: MenuTarget,
    /// Index into the row list (keyboard highlight).
    pub selected: Option<usize>,
}

pub enum ConfirmDialog {
    DiscardFile(FileChange),
    DiscardAll(Vec<FileChange>),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuAction {
    Discard,
    DiscardAll,
    Ignore,
    IgnoreExt,
    CopyPath,
    CopyRelPath,
    Reveal,
    StashAll,
    StashSelected,
    StashApply,
    StashPop,
    StashDrop,
    StashRename,
    StashCopy,
}
