//! The context menu overlay (rows come from `menu_rows`).

use gd_ui::{ContextMenu, MenuEntry, MenuItem};
use gpui::{App, Context, Entity, IntoElement, Pixels, Point, RenderOnce, Window};

use super::menu_rows::menu_rows;
use super::state::MenuAction;
use crate::shared::menu_row::MenuRow;
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct ContextMenuView {
    menu_gen: usize,
    position: Point<Pixels>,
    selected: Option<usize>,
    rows: Vec<MenuRow>,
    ws: Entity<Workspace>,
}

impl ContextMenuView {
    /// `None` when no menu is open.
    pub fn open(ws: &Workspace, cx: &mut Context<Workspace>) -> Option<Self> {
        let menu = ws.menu.as_ref()?;
        Some(Self {
            menu_gen: ws.menu_gen,
            position: menu.position,
            selected: menu.selected,
            rows: menu_rows(&menu.target),
            ws: cx.entity(),
        })
    }
}

impl RenderOnce for ContextMenuView {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let ws = self.ws.clone();
        let mut menu = ContextMenu::new(("context-menu", self.menu_gen), self.position)
            .selected(self.selected)
            .on_dismiss(move |_, cx| ws.update(cx, |this, cx| this.close_menu(cx)));
        for row in self.rows {
            menu = menu.entries([menu_entry(row, &self.ws)]);
        }
        menu
    }
}

fn menu_entry(row: MenuRow, ws: &Entity<Workspace>) -> MenuEntry {
    let MenuRow::Item(r) = row else { return MenuEntry::Separator };
    let ws = ws.clone();
    let action: MenuAction = r.action;
    let mut item = MenuItem::new(r.label)
        .icon(r.icon)
        .disabled(r.disabled)
        .on_select(move |w, cx| ws.update(cx, |this, cx| this.run_menu_action(action, w, cx)));
    if r.danger {
        item = item.danger();
    }
    MenuEntry::Item(item)
}
