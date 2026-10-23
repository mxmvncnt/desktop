use gd_ui::{
    Icon, Theme, ToolbarButton, icon_button,
    theme::TOOLBAR_HEIGHT,
    vdivider,
};
use gpui::{
    App, ClickEvent, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

use super::remote_button::RemoteButton;
use crate::platform::Chrome;
use crate::workspace::{Popover, Workspace};

#[derive(IntoElement)]
pub struct Toolbar {
    ws: Entity<Workspace>,
    chrome: Chrome,
    repo_name: SharedString,
    branch: SharedString,
    remote: RemoteButton,
    popover: Option<Popover>,
}

impl Toolbar {
    pub fn new(ws: &Workspace, ws_entity: Entity<Workspace>, chrome: Chrome, cx: &App) -> Self {
        let repo = ws.repo.as_ref().map(|r| r.read(cx));
        Self {
            ws: ws_entity,
            chrome,
            repo_name: repo.map(|r| r.repo.name().into()).unwrap_or_else(|| "Select a repository".into()),
            branch: repo.map(|r| r.branch_name().into()).unwrap_or_else(|| "—".into()),
            remote: RemoteButton::for_repo(repo),
            popover: ws.popover,
        }
    }
}

impl RenderOnce for Toolbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let chrome = self.chrome;
        let captions = chrome.buttons(window, cx);
        let maximized = chrome.maximized;
        let theme = Theme::of(cx).clone();

        div()
            .flex()
            .flex_none()
            .items_center()
            .h(px(TOOLBAR_HEIGHT))
            .pl(px(6.0 + chrome.leading_inset()))
            .pr(px(6.0))
            .py(px(6.0))
            .gap(px(4.0))
            .bg(theme.surface)
            .rounded_t(px(chrome.radius()))
            .border_b_1()
            .border_color(theme.border)
            .children(captions.and_then(|c| crate::platform::caption_buttons(&c.left, maximized, cx)))
            .child(self.repo_button())
            .child(div().h(px(24.0)).child(vdivider(cx)))
            .child(self.branch_button())
            .child(div().h(px(24.0)).child(vdivider(cx)))
            .child(self.remote_button())
            // Empty toolbar space is the titlebar: drag / double-click / menu.
            .child(crate::platform::titlebar_drag(
                div().id("titlebar-drag").flex_1().h_full(),
                chrome,
            ))
            .child(self.settings_button())
            .children(captions.and_then(|c| crate::platform::caption_buttons(&c.right, maximized, cx)))
    }
}

impl Toolbar {
    fn repo_button(&self) -> impl IntoElement + use<> {
        let ws = self.ws.clone();
        ToolbarButton::new("toolbar-repo", Icon::FolderWithFiles, "Current repository", self.repo_name.clone())
            .chevron()
            .width(280.0)
            .open(self.popover == Some(Popover::Repositories))
            .on_click(trigger_click(ws, Popover::Repositories))
    }

    fn branch_button(&self) -> impl IntoElement + use<> {
        let ws = self.ws.clone();
        ToolbarButton::new("toolbar-branch", Icon::GitBranch, "Current branch", self.branch.clone())
            .chevron()
            .width(260.0)
            .open(matches!(self.popover, Some(Popover::Branches | Popover::NewBranch)))
            .on_click(trigger_click(ws, Popover::Branches))
    }

    fn remote_button(&self) -> impl IntoElement + use<> {
        let ws = self.ws.clone();
        let op = self.remote.op;
        ToolbarButton::new("toolbar-remote", self.remote.icon, self.remote.caption.clone(), self.remote.title.clone())
            .width(240.0)
            .busy(self.remote.busy)
            .on_click(move |_, _, cx| {
                if let Some(op) = op {
                    ws.update(cx, |this, cx| this.remote_action(op, cx))
                }
            })
    }

    fn settings_button(&self) -> impl IntoElement + use<> {
        let ws = self.ws.clone();
        div()
            .id("toolbar-settings-wrap")
            .tooltip(gd_ui::tooltip(format!("Settings ({}+,)", crate::platform::SECONDARY_LABEL)))
            .child(
                icon_button("toolbar-settings", Icon::Settings)
                    .on_click(move |_, _, cx| ws.update(cx, |this, cx| this.toggle_settings(cx))),
            )
    }
}

fn trigger_click(ws: Entity<Workspace>, p: Popover) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    move |ev, window, cx| ws.update(cx, |this, cx| this.click_popover_trigger(p, ev, window, cx))
}
