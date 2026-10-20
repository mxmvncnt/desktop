//! Shown when no repository is selected (Desktop's "Let's get started!").

use gd_ui::{Icon, Theme, motion};
use gpui::{Context, IntoElement, ParentElement, Styled, div, px};

use super::changes::suggestion_card;
use crate::workspace::{Popover, Workspace};

impl Workspace {
    pub fn render_welcome(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = Theme::of(cx).clone();
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .child(motion::fade_in(
                "welcome",
                div()
                    .flex()
                    .flex_col()
                    .gap(px(18.0))
                    .w(px(480.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(22.0))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("Let's get started!"),
                            )
                            .child(
                                div()
                                    .text_color(theme.text_muted)
                                    .child("Add a repository to GitHub Desktop to start collaborating."),
                            ),
                    )
                    .child(suggestion_card(
                        "welcome-add",
                        Icon::Folder,
                        "Add an existing repository",
                        "Pick a folder on this computer (Ctrl+O)",
                        "Choose…",
                        cx.listener(|this, _, _, cx| this.prompt_add_repository(cx)),
                        cx,
                    ))
                    .child(suggestion_card(
                        "welcome-clone",
                        Icon::Cloud,
                        "Clone a repository",
                        "From a URL (GitHub, GitLab, any git remote)",
                        "Clone…",
                        cx.listener(|this, _, w, cx| this.toggle_popover(Popover::Clone, w, cx)),
                        cx,
                    )),
            ))
    }
}
