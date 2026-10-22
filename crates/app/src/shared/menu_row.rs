//! Typed rows of an app context menu (`views/menus.rs` renders them with
//! `gd_ui::ContextMenu`). Features build `Vec<MenuRow>` for their menus.

use gd_ui::Icon;

use crate::views::menus::MenuAction;

pub enum MenuRow {
    Item(RowItem),
    Separator,
}

pub struct RowItem {
    pub action: MenuAction,
    pub label: String,
    pub icon: Icon,
    pub danger: bool,
    pub disabled: bool,
}

impl RowItem {
    pub fn new(action: MenuAction, label: impl Into<String>, icon: Icon) -> Self {
        Self { action, label: label.into(), icon, danger: false, disabled: false }
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl MenuRow {
    pub fn item(action: MenuAction, label: impl Into<String>, icon: Icon) -> Self {
        Self::Item(RowItem::new(action, label, icon))
    }

    /// Keyboard navigation skips separators and disabled items.
    pub fn is_selectable(&self) -> bool {
        matches!(self, Self::Item(i) if !i.disabled)
    }

    /// The action Enter would run on this row.
    pub fn action(&self) -> Option<MenuAction> {
        match self {
            Self::Item(i) if !i.disabled => Some(i.action),
            _ => None,
        }
    }
}
