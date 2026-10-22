//! Arrow / Enter routing across every open menu and dialog. Esc is routed in
//! workspace.rs.

use gpui::{Context, Window};

use super::menu_rows::menu_rows;
use crate::workspace::Workspace;

impl Workspace {
    pub fn menu_step(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.merge_ui.menu.is_some() {
            return self.conflict_menu_step(delta, cx);
        }
        if self.branch_ui.menu.is_some() {
            return self.branch_menu_step(delta, cx);
        }
        let Some(menu) = &mut self.menu else { return };
        let rows = menu_rows(&menu.target);
        let n = rows.len() as isize;
        let mut ix = menu.selected.map(|s| s as isize).unwrap_or(if delta > 0 { -1 } else { n });
        for _ in 0..n {
            ix = (ix + delta).rem_euclid(n);
            if rows[ix as usize].is_selectable() {
                menu.selected = Some(ix as usize);
                break;
            }
        }
        cx.notify();
    }

    /// Enter: run the highlighted menu item, or confirm the open dialog.
    pub fn menu_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.stash_ui.dialog.is_some() {
            self.stash_confirm(cx);
            return;
        }
        if self.conflict_menu_confirm(window, cx) || self.branch_ui_confirm(window, cx) {
            return;
        }
        if self.dialog.is_some() {
            self.confirm_dialog(cx);
            return;
        }
        let Some(menu) = &self.menu else { return };
        let action = menu
            .selected
            .and_then(|s| menu_rows(&menu.target).into_iter().nth(s))
            .and_then(|r| r.action());
        if let Some(a) = action {
            self.run_menu_action(a, window, cx);
        }
    }
}
