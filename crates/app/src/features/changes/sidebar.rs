//! Left sidebar: Changes / History tabs and the selected tab's body.

use gd_ui::{Theme, divider};
use gpui::{
    App, Context, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px,
};

use super::changes_panel::ChangesPanel;
use super::sidebar_handle::SidebarHandle;
use super::sidebar_tabs::SidebarTabs;
use crate::features::history::CommitList;
use crate::repo::{RepoModel, Tab};
use crate::workspace::Workspace;

enum SidebarBody {
    Changes(ChangesPanel),
    History(CommitList),
}

#[derive(IntoElement)]
pub struct Sidebar {
    tabs: SidebarTabs,
    body: SidebarBody,
    corner_radius: f32,
    width: f32,
    handle: SidebarHandle,
}

impl Sidebar {
    pub fn new(ws: &Workspace, repo: &Entity<RepoModel>, cx: &mut Context<Workspace>) -> Self {
        let r = repo.read(cx);
        let (tab, count) = (r.tab, r.changes().len());
        let body = match tab {
            Tab::Changes => SidebarBody::Changes(ChangesPanel::new(ws, repo, cx)),
            Tab::History => SidebarBody::History(CommitList::new(repo.clone(), cx.entity(), ws.history_scroll.clone())),
        };
        Self { tabs: SidebarTabs::new(tab, count, repo.clone()), body, corner_radius: ws.corner_radius,
            width: ws.sidebar_width(),
            handle: SidebarHandle::new(cx.entity()),
        }
    }
}

impl RenderOnce for Sidebar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        div()
            .flex()
            .flex_col()
            .flex_none()
            .relative()
            .w(px(self.width))
            .h_full()
            .bg(theme.surface)
            .rounded_bl(px(self.corner_radius))
            .border_r_1()
            .border_color(theme.border)
            .child(div().p(px(8.0)).child(self.tabs))
            .child(divider(cx))
            .child(self.handle)
            .child(match self.body {
                SidebarBody::Changes(p) => p.into_any_element(),
                SidebarBody::History(l) => l.into_any_element(),
            })
    }
}

