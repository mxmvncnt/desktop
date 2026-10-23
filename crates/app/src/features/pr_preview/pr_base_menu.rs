//! Dropdown listing the branches a pull request can target.

use gd_git::BranchKind;
use gd_ui::{Icon, Theme, icon, overlay_card, pressable};
use gpui::{App, Entity, IntoElement, MouseButton, RenderOnce, Window, deferred, div, prelude::*, px, uniform_list};

use crate::repo::RepoModel;

const ROW: f32 = 28.0;
const MAX_ROWS: usize = 9;

#[derive(IntoElement)]
pub struct PrBaseMenu {
    repo: Entity<RepoModel>,
}

impl PrBaseMenu {
    /// `None` while the dropdown is closed.
    pub fn new(repo: &Entity<RepoModel>, cx: &App) -> Option<Self> {
        repo.read(cx).pr.base_menu.then(|| Self { repo: repo.clone() })
    }
}

impl RenderOnce for PrBaseMenu {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let r = self.repo.read(cx);
        let head = r.branch_name();
        let upstream = r.status.as_ref().and_then(|s| s.upstream.clone());
        let names: Vec<(String, bool)> = r
            .branches
            .iter()
            .filter(|b| b.name != head && Some(&b.name) != upstream.as_ref())
            .map(|b| (b.name.clone(), b.kind == BranchKind::Remote))
            .collect();
        let chosen = r.pr.base.clone();
        let scroll = r.pr.base_scroll.clone();
        let height = ROW * names.len().clamp(1, MAX_ROWS) as f32 + 8.0;
        let repo = self.repo;
        let list = uniform_list("pr-base-list", names.len(), move |range, _w, cx| {
            let theme = Theme::of(cx).clone();
            range
                .filter_map(|ix| {
                    let (name, remote) = names.get(ix)?.clone();
                    let (repo, picked) = (repo.clone(), name.clone());
                    Some(
                        pressable(("pr-base", ix), chosen.as_deref() == Some(&name), cx)
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .h(px(ROW))
                            .px(px(8.0))
                            .rounded(px(6.0))
                            .child(icon(if remote { Icon::Cloud } else { Icon::GitBranch }).size(px(13.0)).text_color(theme.text_faint))
                            .child(div().truncate().child(name))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                repo.update(cx, |r, cx| r.set_pr_base(picked.clone(), cx))
                            }),
                    )
                })
                .collect()
        })
        .track_scroll(&scroll)
        .px(px(4.0))
        .py(px(4.0))
        .h(px(height));
        deferred(div().absolute().top(px(32.0)).left_0().w(px(300.0)).child(overlay_card(cx).child(list))).priority(2)
    }
}
