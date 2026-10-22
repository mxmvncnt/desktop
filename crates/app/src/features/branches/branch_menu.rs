//! The branch context menu overlay (rows come from `branch_menu_rows`).

use gd_ui::{ContextMenu, MenuEntry, MenuItem};
use gpui::{App, Context, Entity, IntoElement, Pixels, Point, RenderOnce, Window};

use super::branch_menu_rows::{BranchRow, branch_menu_rows};
use super::state::BranchMenu;
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct BranchMenuView {
    menu_gen: usize,
    position: Point<Pixels>,
    selected: Option<usize>,
    rows: Vec<BranchRow>,
    ws: Entity<Workspace>,
}

impl BranchMenuView {
    /// `None` when no branch menu is open.
    pub fn open(ws: &Workspace, cx: &mut Context<Workspace>) -> Option<Self> {
        let BranchMenu { position, selected, branch } = ws.branch_ui.menu.as_ref()?;
        Some(Self {
            menu_gen: ws.branch_ui.menu_gen,
            position: *position,
            selected: *selected,
            rows: branch_menu_rows(branch, ws.paused(cx)),
            ws: cx.entity(),
        })
    }
}

impl RenderOnce for BranchMenuView {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let ws = self.ws.clone();
        let mut menu = ContextMenu::new(("merge-menu", self.menu_gen), self.position)
            .selected(self.selected)
            .on_dismiss(move |_, cx| {
                ws.update(cx, |this, cx| {
                    this.close_branch_menu();
                    cx.notify()
                })
            });
        for row in self.rows {
            menu = menu.entries([menu_entry(row, &self.ws)]);
        }
        menu
    }
}

fn menu_entry(row: BranchRow, ws: &Entity<Workspace>) -> MenuEntry {
    let BranchRow::Item(r) = row else { return MenuEntry::Separator };
    let ws = ws.clone();
    let action = r.action;
    let mut item = MenuItem::new(r.label)
        .icon(r.icon)
        .disabled(r.disabled)
        .on_select(move |w, cx| ws.update(cx, |this, cx| this.run_branch_action(action, w, cx)));
    if r.danger {
        item = item.danger();
    }
    MenuEntry::Item(item)
}
