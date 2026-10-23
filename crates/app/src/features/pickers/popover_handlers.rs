//! `impl Workspace`: opening, toggling and closing the toolbar popovers.

use gpui::{ClickEvent, Context, Focusable, ScrollStrategy, Window};

use crate::workspace::{Popover, Workspace};

impl Workspace {
    /// Toolbar trigger click: toggles `p`, unless this press is the one that
    /// just closed it through the backdrop.
    pub(crate) fn click_popover_trigger(
        &mut self,
        p: Popover,
        ev: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let dismissed = self.popover_dismissed.take();
        if let (Some((was, at)), ClickEvent::Mouse(m)) = (dismissed, ev)
            && popover_trigger(was) == p
            && at == m.down.position
        {
            return;
        }
        self.toggle_popover(p, window, cx)
    }

    pub fn toggle_popover(&mut self, p: Popover, window: &mut Window, cx: &mut Context<Self>) {
        if self.popover == Some(p) {
            self.close_popover(window, cx);
            return;
        }
        if matches!(p, Popover::Branches | Popover::NewBranch) && self.repo.is_none() {
            return;
        }
        self.popover = Some(p);
        self.popover_gen += 1;
        self.new_branch_base = None;
        self.new_branch_base_open = false;
        self.reset_popover_selection(cx);
        for input in [&self.filter, &self.name_input, &self.url_input] {
            input.update(cx, |s, cx| s.emplace("", cx));
        }
        let target = match p {
            Popover::Repositories | Popover::Branches => &self.filter,
            Popover::NewBranch => &self.name_input,
            Popover::Clone => &self.url_input,
        };
        window.focus(&target.focus_handle(cx), cx);
        cx.notify();
    }

    /// Put the picker cursor on the open repository while the repo filter is
    /// empty (a settings index is then also a list row), else on the first row.
    /// Runs on open and on every input change, including the clear on open.
    pub(crate) fn reset_popover_selection(&mut self, cx: &mut Context<Self>) {
        let unfiltered = self.filter.read(cx).as_str().trim().is_empty();
        self.popover_selected = match self.popover {
            Some(Popover::Repositories) if unfiltered => self.settings.selected.unwrap_or(0),
            _ => 0,
        };
        self.popover_scroll
            .scroll_to_item(self.popover_selected, ScrollStrategy::Nearest);
    }

    pub fn close_popover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.popover = None;
        self.focus(window, cx);
        cx.notify();
    }

    /// Up/down inside an open popover.
    pub(crate) fn step_popover_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        self.popover_selected = (self.popover_selected as isize + delta).max(0) as usize;
        self.popover_scroll
            .scroll_to_item(self.popover_selected, ScrollStrategy::Nearest);
        cx.notify();
    }
}

/// The toolbar button that owns a popover.
fn popover_trigger(p: Popover) -> Popover {
    match p {
        Popover::Repositories | Popover::Clone => Popover::Repositories,
        Popover::Branches | Popover::NewBranch => Popover::Branches,
    }
}
