//! "Git identity" section: the global identity, and the override for the open repository.

use gd_git::{EffectiveIdentity, IdentitySource};
use gd_ui::{Button, Theme};
use gpui::{App, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};

use super::identity_block::IdentityBlock;
use super::state::{AccountInputs, IdentityState};
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct IdentitySection {
    identity: IdentityState,
    inputs: AccountInputs,
    /// Name of the open repository (`None` = no override editor).
    repo_name: Option<String>,
    ws: Entity<Workspace>,
}

impl IdentitySection {
    pub fn new(identity: IdentityState, inputs: AccountInputs, repo_name: Option<String>, ws: Entity<Workspace>) -> Self {
        Self { identity, inputs, repo_name, ws }
    }

    fn global_block(&self) -> IdentityBlock {
        let ws = self.ws.clone();
        let inp = &self.inputs;
        // Global message only when the last action was not about the repository.
        IdentityBlock::new(
            "Git identity",
            "Used for your commits in every repository (global git config: user.name / user.email).",
            ("identity-global-name", "identity-global-email"),
            (&inp.global_name, &inp.global_email),
        )
        .button(Button::new("identity-global-save").primary().compact().label("Save").on_click(move |_, _, cx| {
            ws.update(cx, |this, cx| this.save_global_identity(cx))
        }))
        .status(self.identity.message.clone())
    }

    fn local_block(&self, repo_name: &str) -> IdentityBlock {
        let (ws_save, ws_clear) = (self.ws.clone(), self.ws.clone());
        let inp = &self.inputs;
        IdentityBlock::new(
            format!("Override for {repo_name}"),
            "Optional. Leave empty to use the global identity in this repository (local git config).",
            ("identity-local-name", "identity-local-email"),
            (&inp.local_name, &inp.local_email),
        )
        .button(Button::new("identity-local-save").compact().label("Save override").on_click(move |_, _, cx| {
            ws_save.update(cx, |this, cx| this.save_local_identity(cx))
        }))
        .button(
            Button::new("identity-local-clear")
                .ghost()
                .compact()
                .disabled(self.identity.local.is_empty())
                .label("Remove override")
                .on_click(move |_, _, cx| ws_clear.update(cx, |this, cx| this.clear_local_identity(cx))),
        )
    }
}

impl RenderOnce for IdentitySection {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let mut col = div().flex().flex_col().gap(px(14.0)).child(self.global_block());
        if let Some(name) = &self.repo_name {
            col = col.child(self.local_block(name));
            if let Some(e) = &self.identity.effective {
                col = col.child(div().text_size(px(12.0)).text_color(theme.text_muted).child(effective_line(e)));
            }
        } else {
            col = col.child(
                div().text_size(px(12.0)).text_color(theme.text_faint).child("Open a repository to set a per-repository override."),
            );
        }
        col
    }
}

/// "In effect here: Name <email> (repository override)".
fn effective_line(e: &EffectiveIdentity) -> String {
    let who = match (e.identity.name.as_str(), e.identity.email.as_str()) {
        ("", "") => "no identity configured".to_string(),
        (n, "") => n.to_string(),
        ("", m) => format!("<{m}>"),
        (n, m) => format!("{n} <{m}>"),
    };
    let from = match e.source {
        IdentitySource::Repository => "repository override",
        IdentitySource::Global => "global",
        IdentitySource::Unset => "unset",
    };
    format!("In effect here: {who} ({from})")
}
