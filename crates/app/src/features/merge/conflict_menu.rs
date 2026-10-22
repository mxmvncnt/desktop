//! The conflicted-file context menu overlay (rows come from `conflict_menu_rows`).

use gd_ui::{ContextMenu, MenuEntry, MenuItem};
use gpui::{App, Context, Entity, IntoElement, Pixels, Point, RenderOnce, Window};

use super::conflict_menu_rows::{ConflictRow, conflict_menu_rows};
use super::state::ConflictMenu;
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct ConflictMenuView {
    menu_gen: usize,
    position: Point<Pixels>,
    selected: Option<usize>,
    rows: Vec<ConflictRow>,
    ws: Entity<Workspace>,
}

impl ConflictMenuView {
    /// `None` when no conflict menu is open.
    pub fn open(ws: &Workspace, cx: &mut Context<Workspace>) -> Option<Self> {
        let ConflictMenu { position, selected, .. } = ws.merge_ui.menu.as_ref()?;
        Some(Self {
            menu_gen: ws.merge_ui.menu_gen,
            position: *position,
            selected: *selected,
            rows: conflict_menu_rows(),
            ws: cx.entity(),
        })
    }
}

impl RenderOnce for ConflictMenuView {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let ws = self.ws.clone();
        let mut menu = ContextMenu::new(("merge-menu", self.menu_gen), self.position)
            .selected(self.selected)
            .on_dismiss(move |_, cx| {
                ws.update(cx, |this, cx| {
                    this.merge_ui.menu = None;
                    cx.notify()
                })
            });
        for row in self.rows {
            menu = menu.entries([menu_entry(row, &self.ws)]);
        }
        menu
    }
}

fn menu_entry(row: ConflictRow, ws: &Entity<Workspace>) -> MenuEntry {
    let ConflictRow::Item(r) = row else { return MenuEntry::Separator };
    let ws = ws.clone();
    let action = r.action;
    let mut item = MenuItem::new(r.label)
        .icon(r.icon)
        .disabled(r.disabled)
        .on_select(move |w, cx| ws.update(cx, |this, cx| this.run_conflict_action(action, w, cx)));
    if r.danger {
        item = item.danger();
    }
    MenuEntry::Item(item)
}
