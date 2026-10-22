use gd_ui::Tabs;
use gpui::{App, Entity, IntoElement, RenderOnce, SharedString, Window};

use crate::repo::{RepoModel, Tab};

#[derive(IntoElement)]
pub struct SidebarTabs {
    tab: Tab,
    change_count: usize,
    repo: Entity<RepoModel>,
}

impl SidebarTabs {
    pub fn new(tab: Tab, change_count: usize, repo: Entity<RepoModel>) -> Self {
        Self { tab, change_count, repo }
    }
}

impl RenderOnce for SidebarTabs {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let repo = self.repo;
        let n = self.change_count;
        Tabs::new("sidebar-tabs", if self.tab == Tab::Changes { 0 } else { 1 })
            .tab("Changes", (n > 0).then(|| SharedString::from(n.to_string())))
            .tab("History", None)
            .on_select(move |ix, _, cx| {
                repo.update(cx, |r, cx| r.set_tab(if ix == 0 { Tab::Changes } else { Tab::History }, cx))
            })
    }
}
