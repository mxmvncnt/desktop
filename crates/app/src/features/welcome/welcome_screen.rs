//! Desktop's "Let's get started!" screen.

use gd_ui::{Icon, Theme, motion};
use gpui::{App, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};

use crate::shared::suggestion_card::SuggestionCard;
use crate::workspace::{Popover, Workspace};

#[derive(IntoElement)]
pub struct WelcomeScreen {
    ws: Entity<Workspace>,
}

impl WelcomeScreen {
    pub fn new(ws: Entity<Workspace>) -> Self {
        Self { ws }
    }

    fn add_card(&self) -> SuggestionCard {
        let ws = self.ws.clone();
        SuggestionCard::new(
            "welcome-add",
            Icon::Folder,
            "Add an existing repository",
            format!("Pick a folder on this computer ({}+O)", crate::platform::SECONDARY_LABEL),
            "Choose…",
            move |_, _, cx| ws.update(cx, |this, cx| this.prompt_add_repository(cx)),
        )
    }

    fn clone_card(&self) -> SuggestionCard {
        let ws = self.ws.clone();
        SuggestionCard::new(
            "welcome-clone",
            Icon::Cloud,
            "Clone a repository",
            "From a URL (GitHub, GitLab, any git remote)",
            "Clone…",
            move |_, w, cx| ws.update(cx, |this, cx| this.toggle_popover(Popover::Clone, w, cx)),
        )
    }
}

impl RenderOnce for WelcomeScreen {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let title = div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(div().text_size(px(22.0)).font_weight(gpui::FontWeight::SEMIBOLD).child("Let's get started!"))
            .child(
                div()
                    .text_color(theme.text_muted)
                    .child("Add a repository to desktop-rs to start collaborating."),
            );
        div().flex_1().flex().items_center().justify_center().child(motion::fade_in(
            "welcome",
            div()
                .flex()
                .flex_col()
                .gap(px(18.0))
                .w(px(480.0))
                .child(title)
                .child(self.add_card())
                .child(self.clone_card()),
        ))
    }
}
