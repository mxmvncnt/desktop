//! The commit context menu overlay (rows come from `commit_menu_rows`).

use gd_ui::{ContextMenu, MenuEntry, MenuItem};
use gpui::{App, Context, Entity, IntoElement, RenderOnce, Window};

use super::commit_menu_rows::{HistoryRow, commit_menu_rows};
use super::state::HistoryMenu;
use crate::workspace::Workspace;

#[derive(IntoElement)]
pub struct CommitMenuView {
    menu_gen: usize,
    position: gpui::Point<gpui::Pixels>,
    selected: Option<usize>,
    rows: Vec<HistoryRow>,
    ws: Entity<Workspace>,
}

impl CommitMenuView {
    /// `None` when no commit menu is open.
    pub fn open(ws: &Workspace, cx: &mut Context<Workspace>) -> Option<Self> {
        let HistoryMenu { position, selected, ctx } = ws.history_ui.menu.as_ref()?;
        Some(Self {
            menu_gen: ws.history_ui.menu_gen,
            position: *position,
            selected: *selected,
            rows: commit_menu_rows(ctx),
            ws: cx.entity(),
        })
    }
}

impl RenderOnce for CommitMenuView {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let ws = self.ws.clone();
        let mut menu = ContextMenu::new(("hist-menu", self.menu_gen), self.position)
            .selected(self.selected)
            .on_dismiss(move |w, cx| {
                ws.update(cx, |this, cx| {
                    this.hist_dismiss(w, cx);
                })
            });
        for row in self.rows {
            menu = menu.entries([menu_entry(row, &self.ws)]);
        }
        menu
    }
}

fn menu_entry(row: HistoryRow, ws: &Entity<Workspace>) -> MenuEntry {
    let HistoryRow::Item(r) = row else { return MenuEntry::Separator };
    let ws = ws.clone();
    let action = r.action;
    let mut item = MenuItem::new(r.label)
        .icon(r.icon)
        .disabled(r.disabled)
        .on_select(move |w, cx| ws.update(cx, |this, cx| this.run_hist_action(action.clone(), w, cx)));
    if r.danger {
        item = item.danger();
    }
    MenuEntry::Item(item)
}
