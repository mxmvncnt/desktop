//! The root "template": toolbar, body (sidebar + main pane or welcome) and
//! the overlay stack.

use gd_ui::Theme;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
    div, px,
};

use super::{actions, state::Workspace};
use crate::actions::WORKSPACE;
use crate::features::branches::{BranchDialogView, BranchMenuView};
use crate::features::changes::Sidebar;
use crate::features::diff::DiffOptionsMenu;
use crate::features::history::{CommitMenuView, HistoryDialogView};
use crate::features::menus::{ContextMenuView, DiscardDialogView};
use crate::features::merge::{ConflictMenuView, InProgressBanner, MergeDialogView};
use crate::features::notices::NoticeBanner;
use crate::features::pickers::PopoverLayer;
use crate::features::settings::SettingsModal;
use crate::features::stashes::StashDialogView;
use crate::features::toolbar::Toolbar;
use crate::features::welcome::WelcomeScreen;
use crate::platform::Chrome;
use crate::repo::Tab;

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        crate::frame_debug::on_render();
        let chrome = Chrome::detect(window);
        self.corner_radius = chrome.radius();
        let body = self.body(cx);
        let overlays = self.overlays(cx);
        let in_progress = InProgressBanner::open(self, cx);
        let toolbar = Toolbar::new(self, cx.entity(), chrome, cx);

        let root = div()
            .id("workspace")
            .on_mouse_move(|_, _, _| crate::frame_debug::on_input())
            .key_context(if self.overlay_open() { "Workspace Menu" } else { WORKSPACE })
            .track_focus(&self.focus);
        let root = actions::bind(root, cx)
            .size_full()
            .flex()
            .flex_col()
            .relative()
            .bg(theme.bg)
            .font_family(theme.font_sans.clone())
            .text_size(px(13.0))
            .text_color(theme.text)
            .rounded(px(chrome.radius()))
            .child(toolbar)
            .children(in_progress)
            .child(body)
            .children(overlays);
        crate::platform::frame(root.into_any_element(), chrome, window, cx)
    }
}

impl Workspace {
    /// Sidebar + main pane for the open repo, or the welcome screen.
    fn body(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(repo) = self.repo.clone() else {
            return WelcomeScreen::new(cx.entity()).into_any_element();
        };
        let tab = repo.read(cx).tab;
        div()
            .flex()
            .flex_1()
            .min_h_0()
            .child(Sidebar::new(self, &repo, cx))
            .child(match tab {
                Tab::Changes => self.render_changes_main(&repo, cx).into_any_element(),
                Tab::History => self.render_history_main(&repo, cx).into_any_element(),
            })
            .into_any_element()
    }

    /// Banner, popovers, menus and dialogs, in z-order (last paints on top).
    fn overlays(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let banner = NoticeBanner::open(self, cx);
        let popover = PopoverLayer::open(self, cx);
        let menu = ContextMenuView::open(self, cx);
        let diff_options = DiffOptionsMenu::new(self, cx.entity());
        let dialog = DiscardDialogView::open(self, cx);
        let settings = SettingsModal::open(self, cx);
        let hist_menu = CommitMenuView::open(self, cx);
        let hist_dialog = HistoryDialogView::open(self, cx);
        let stash_dialog = StashDialogView::open(self, cx);
        let merge_overlays = div()
            .absolute()
            .inset_0()
            .children(BranchMenuView::open(self, cx))
            .children(ConflictMenuView::open(self, cx))
            .children(MergeDialogView::open(self, cx))
            .children(BranchDialogView::open(self, cx));

        let mut out = Vec::new();
        out.extend(banner.map(IntoElement::into_any_element));
        out.extend(popover.map(IntoElement::into_any_element));
        out.extend(menu.map(IntoElement::into_any_element));
        out.extend(diff_options.map(IntoElement::into_any_element));
        out.extend(dialog.map(IntoElement::into_any_element));
        out.extend(settings.map(IntoElement::into_any_element));
        out.extend(stash_dialog.map(IntoElement::into_any_element));
        out.extend(hist_menu.map(IntoElement::into_any_element));
        out.extend(hist_dialog.map(IntoElement::into_any_element));
        out.push(merge_overlays.into_any_element());
        out
    }
}
