//! Changes tab of the sidebar: select-all header, file list, stashes, undo
//! strip and the commit box.

use gd_ui::divider;
use gpui::{
    App, Context, Entity, IntoElement, InteractiveElement, ParentElement, RenderOnce, Styled, Window, div, px,
};

use super::changes_header::ChangesHeader;
use super::changes_list::ChangesList;
use super::commit_box::CommitBox;
use super::state::{CommitButton, SelectionSummary};
use super::undo_strip::UndoStrip;
use crate::actions::LIST;
use crate::features::stashes::StashPanel;
use crate::repo::{LastCommit, RepoModel};
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct ChangesPanel {
    header: ChangesHeader,
    list: ChangesList,
    stashes: Option<StashPanel>,
    undo: Option<UndoStrip>,
    commit_box: CommitBox,
}

impl ChangesPanel {
    pub fn new(ws: &Workspace, repo: &Entity<RepoModel>, cx: &mut Context<Workspace>) -> Self {
        let r = repo.read(cx);
        let selection = SelectionSummary::of(r);
        let amending = ws.amending(cx);
        let summary_ok = !ws.summary.read(cx).as_str().trim().is_empty();
        let button = CommitButton::of(r, selection.included, amending, summary_ok);
        let last: Option<LastCommit> = r.undoable_commit().cloned().filter(|_| !amending);
        let ws_entity = cx.entity();
        Self {
            header: ChangesHeader::new(selection, repo.clone(), ws_entity.clone()),
            list: ChangesList::new(selection.total, repo.clone(), ws_entity.clone(), ws.changes_scroll.clone()),
            stashes: StashPanel::new(repo, ws_entity.clone(), cx),
            undo: last.map(|c| UndoStrip::new(c, ws_entity.clone())),
            commit_box: CommitBox::new(button, ws.summary.clone(), ws.description.clone(), ws_entity),
        }
    }
}

impl RenderOnce for ChangesPanel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(self.header)
            .child(divider(cx))
            .child(div().key_context(LIST).flex().flex_col().flex_1().min_h_0().py(px(4.0)).child(self.list))
            .children(self.stashes)
            .children(self.undo)
            .child(self.commit_box)
    }
}

