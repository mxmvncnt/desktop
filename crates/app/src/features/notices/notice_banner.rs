use gd_ui::{Icon, Theme, icon, motion};
use gpui::{
    App, Entity, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window, div, px,
};

use crate::workspace::Workspace;

#[derive(Clone, Copy, PartialEq, Eq)]
enum NoticeKind {
    Error,
    Success,
}

#[derive(IntoElement)]
pub struct NoticeBanner {
    ws: Entity<Workspace>,
    kind: NoticeKind,
    message: String,
}

impl NoticeBanner {
    /// `None` when there is nothing to show. Errors win over notices.
    pub fn open(ws: &Workspace, cx: &mut gpui::Context<Workspace>) -> Option<Self> {
        let (error, notice) = match &ws.repo {
            Some(r) => {
                let r = r.read(cx);
                (r.error.clone(), r.notice.clone())
            }
            None => (ws.startup_error.clone(), None),
        };
        let (kind, message) = match (error, notice) {
            (Some(e), _) => (NoticeKind::Error, e),
            (None, Some(n)) => (NoticeKind::Success, n),
            _ => return None,
        };
        Some(Self { ws: cx.entity(), kind, message })
    }
}

impl RenderOnce for NoticeBanner {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let is_error = self.kind == NoticeKind::Error;
        let key = format!("banner-{}", hash(&self.message));
        let ws = self.ws;
        div()
            .absolute()
            .bottom(px(16.0))
            .right(px(16.0))
            .max_w(px(520.0))
            .child(motion::fade_in(
                SharedString::from(key),
                gd_ui::overlay_card(cx)
                    .flex()
                    .items_start()
                    .gap(px(10.0))
                    .p(px(12.0))
                    .child(
                        icon(if is_error { Icon::Warning } else { Icon::Check })
                            .text_color(if is_error { theme.danger } else { theme.success }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.5))
                            .text_color(theme.text)
                            .child(self.message),
                    )
                    .child(
                        gd_ui::icon_button("dismiss-banner", Icon::Close)
                            .compact()
                            .on_click(move |_, _, cx| ws.update(cx, |this, cx| this.dismiss_notices(cx))),
                    ),
            ))
    }
}

fn hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}
