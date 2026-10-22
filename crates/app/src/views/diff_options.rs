//! Diff display options: the gear button in the diff headers and its popover
//! (Unified / Split, "Wrap long lines", "Hide whitespace changes"). All options are app-wide and
//! persisted in `Settings`; the whitespace flag is also pushed into
//! `RepoModel::diff_options`, which reloads the visible diffs.

use std::sync::Arc;

use gd_git::{Diff, DiffOptions};
use gd_ui::{CheckState, Checkbox, Icon, Tabs, Theme, icon, icon_button, motion, pressable};
use gpui::{
    Context, Entity, InteractiveElement, IntoElement, ListAlignment, ListState, MouseButton,
    ParentElement, Pixels, Point, SharedString, Styled, Window, deferred, div, point, prelude::*, px,
};

use super::diff_split::SplitDiff;
use crate::workspace::Workspace;

const MENU_W: f32 = 264.0;

/// Workspace-side state for the diff options (one field on `Workspace`).
pub struct DiffView {
    /// Open popover position (window coordinates).
    pub menu: Option<Point<Pixels>>,
    pub menu_gen: usize,
    /// Paired rows of the diff currently shown in split mode.
    split_cache: Option<Arc<SplitDiff>>,
    /// Horizontal scroll of the diff text (both split columns), see `TextFlow`.
    pub hscroll: Pixels,
    /// What `Workspace::scroll_diff_x` needs to clamp `hscroll`.
    pub extent: Option<HExtent>,
    /// Variable-height list used while lines wrap, and the diff (and split
    /// mode) it was last reset for.
    wrap_list: ListState,
    wrap_for: Option<(Arc<Diff>, bool)>,
}

impl Default for DiffView {
    fn default() -> Self {
        Self {
            menu: None,
            menu_gen: 0,
            split_cache: None,
            hscroll: px(0.0),
            extent: None,
            wrap_list: ListState::new(0, ListAlignment::Top, px(400.0)),
            wrap_for: None,
        }
    }
}

/// Horizontal layout of the shown diff: the text columns share the list
/// width minus `fixed` (gutters, markers) evenly.
pub struct HExtent {
    pub diff: Arc<Diff>,
    pub fixed: Pixels,
    pub columns: f32,
}

impl DiffView {
    pub fn set_extent(&mut self, diff: &Arc<Diff>, fixed: Pixels, columns: f32) {
        self.extent = Some(HExtent { diff: diff.clone(), fixed, columns });
    }

    /// The wrapping list's state, reset when the diff or split mode changed.
    /// A reload of the same file (`!new_key`) stays at the same row.
    pub fn wrap_list_for(&mut self, diff: &Arc<Diff>, split: bool, n: usize, new_key: bool) -> ListState {
        let same = self.wrap_for.as_ref().is_some_and(|(d, s)| Arc::ptr_eq(d, diff) && *s == split);
        if !same || new_key {
            let top = self.wrap_list.logical_scroll_top();
            let keep = !new_key && self.wrap_for.as_ref().is_some_and(|(_, s)| *s == split);
            self.wrap_list.reset(n);
            if keep && top.item_ix < n {
                self.wrap_list.scroll_to(top);
            }
            self.wrap_for = Some((diff.clone(), split));
        }
        self.wrap_list.clone()
    }
}

impl Workspace {
    /// The gear button for a diff header.
    pub fn diff_options_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ws = cx.entity();
        icon_button("diff-options", Icon::Tuning).compact().on_click(move |e, w, cx| {
            let pos = e.position();
            ws.update(cx, |this, cx| this.open_diff_options(pos, w, cx))
        })
    }

    pub fn open_diff_options(&mut self, pos: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if self.diff_view.menu.take().is_some() {
            cx.notify();
            return;
        }
        self.popover = None;
        self.menu = None;
        self.diff_view.menu = Some(pos);
        self.diff_view.menu_gen += 1;
        self.focus(window, cx);
        cx.notify();
    }

    pub fn close_diff_options(&mut self, cx: &mut Context<Self>) {
        if self.diff_view.menu.take().is_some() {
            cx.notify();
        }
    }

    pub fn set_diff_split(&mut self, split: bool, cx: &mut Context<Self>) {
        if self.settings.diff_split == split {
            return;
        }
        self.settings.diff_split = split;
        self.settings.save();
        // Row indices mean something else in the other mode.
        self.diff_scroll.0.borrow().base_handle.set_offset(point(px(0.0), px(0.0)));
        self.diff_view.hscroll = px(0.0);
        cx.notify();
    }

    pub fn set_diff_wrap(&mut self, wrap: bool, cx: &mut Context<Self>) {
        if self.settings.diff_wrap == wrap {
            return;
        }
        self.settings.diff_wrap = wrap;
        self.settings.save();
        // The other mode uses another list; start both at the top.
        self.diff_scroll.0.borrow().base_handle.set_offset(point(px(0.0), px(0.0)));
        self.diff_view.wrap_for = None;
        cx.notify();
    }

    pub fn set_ignore_whitespace(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.settings.diff_ignore_whitespace == on {
            return;
        }
        self.settings.diff_ignore_whitespace = on;
        self.settings.save();
        self.apply_diff_options(cx);
        cx.notify();
    }

    /// Push the persisted options into the repository model (reloads diffs if changed).
    pub fn apply_diff_options(&mut self, cx: &mut Context<Self>) {
        let opts = DiffOptions { ignore_whitespace: self.settings.diff_ignore_whitespace };
        if let Some(repo) = &self.repo {
            repo.update(cx, |r, cx| r.set_diff_options(opts, cx));
        }
    }

    /// Paired rows for `diff`, built once per loaded diff and cached.
    pub fn split_for(&mut self, diff: &Arc<Diff>) -> Arc<SplitDiff> {
        if let Some(s) = &self.diff_view.split_cache {
            if Arc::ptr_eq(&s.diff, diff) {
                return s.clone();
            }
        }
        let s = Arc::new(SplitDiff::build(diff));
        self.diff_view.split_cache = Some(s.clone());
        s
    }

    /// Small note shown under the diff header while whitespace is hidden.
    pub fn whitespace_note(&self, cx: &gpui::App) -> Option<impl IntoElement + use<>> {
        if !self.settings.diff_ignore_whitespace {
            return None;
        }
        let theme = Theme::of(cx);
        Some(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(6.0))
                .h(px(24.0))
                .px(px(16.0))
                .border_b_1()
                .border_color(theme.border)
                .bg(gd_ui::theme::ink(0.02))
                .text_size(px(12.0))
                .text_color(theme.text_faint)
                .child(icon(Icon::EyeClosed).size(px(13.0)).text_color(theme.text_faint))
                .child("Whitespace changes are hidden"),
        )
    }

    pub fn render_diff_options_menu(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let pos = self.diff_view.menu?;
        let theme = Theme::of(cx).clone();
        let inset = window.client_inset().unwrap_or(px(0.0));
        let view = window.viewport_size();
        let x = (pos.x - inset - px(MENU_W - 24.0))
            .min(view.width - inset * 2.0 - px(MENU_W + 8.0))
            .max(px(8.0));
        let y = pos.y - inset + px(18.0);
        let split = self.settings.diff_split;
        let ws_hidden = self.settings.diff_ignore_whitespace;
        let wrap = self.settings.diff_wrap;
        let ws = cx.entity();
        let ws_tabs = ws.clone();
        let ws_dismiss = ws.clone();

        let label = |t: &'static str| {
            div()
                .px(px(8.0))
                .pt(px(4.0))
                .pb(px(6.0))
                .text_size(px(11.5))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.text_faint)
                .child(t)
        };
        let card = gd_ui::overlay_card(cx)
            .id("diff-options-card")
            .occlude()
            .w(px(MENU_W))
            .p(px(6.0))
            .flex()
            .flex_col()
            .child(label("Diff display"))
            .child(
                div().px(px(2.0)).pb(px(6.0)).child(
                    Tabs::new("diff-display", usize::from(split))
                        .tab("Unified", None)
                        .tab("Split", None)
                        .on_select(move |ix, _, cx| {
                            ws_tabs.update(cx, |this, cx| this.set_diff_split(ix == 1, cx))
                        }),
                ),
            )
            .child(option_row("diff-opt-wrap", "Wrap long lines", wrap, ws.clone(), move |this, cx| {
                this.set_diff_wrap(!wrap, cx)
            }, cx))
            .child(div().h(px(1.0)).mx(px(2.0)).my(px(4.0)).bg(theme.border))
            .child(label("Whitespace"))
            .child(option_row("diff-opt-ws", "Hide whitespace changes", ws_hidden, ws, move |this, cx| {
                this.set_ignore_whitespace(!ws_hidden, cx)
            }, cx));
        let backdrop = div()
            .id("diff-options-backdrop")
            .absolute()
            .inset_0()
            .on_mouse_down(MouseButton::Left, {
                let ws = ws_dismiss.clone();
                move |_, _, cx| ws.update(cx, |this, cx| this.close_diff_options(cx))
            })
            .on_mouse_down(MouseButton::Right, move |_, _, cx| {
                ws_dismiss.update(cx, |this, cx| this.close_diff_options(cx))
            });
        Some(deferred(
            div().absolute().inset_0().child(backdrop).child(
                div().absolute().left(x).top(y).child(motion::menu_in(
                    SharedString::from(format!("diff-options-in-{}", self.diff_view.menu_gen)),
                    card,
                )),
            ),
        ))
    }
}

/// A checkbox row of the options popover; clicking the box or the row toggles.
pub(super) fn option_row(
    id: &'static str,
    text: &'static str,
    checked: bool,
    ws: Entity<Workspace>,
    toggle: impl Fn(&mut Workspace, &mut Context<Workspace>) + Clone + 'static,
    cx: &mut Context<Workspace>,
) -> gpui::Stateful<gpui::Div> {
    let (ws_check, toggle_check) = (ws.clone(), toggle.clone());
    pressable(id, false, cx)
        .flex()
        .items_center()
        .gap(px(10.0))
        .h(px(30.0))
        .px(px(8.0))
        .rounded(px(6.0))
        .text_size(px(13.0))
        .child(
            Checkbox::new(
                SharedString::from(format!("{id}-check")),
                if checked { CheckState::Checked } else { CheckState::Unchecked },
            )
            .on_click(move |_, _, cx| ws_check.update(cx, |this, cx| toggle_check(this, cx))),
        )
        .child(text)
        .on_click(move |_, _, cx| ws.update(cx, |this, cx| toggle(this, cx)))
}
