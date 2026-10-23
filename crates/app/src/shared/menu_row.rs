//! Typed rows of an app context menu (`features/menus` renders them with
//! `gd_ui::ContextMenu`). Features build `Vec<MenuRow>` for their menus.
//! `A` is the action type: `MenuAction` for the generic menu, a feature's own
//! enum for feature-owned menus (history).

use gd_ui::Icon;

use crate::features::menus::MenuAction;

pub enum MenuRow<A = MenuAction> {
    Item(RowItem<A>),
    Separator,
}

pub struct RowItem<A = MenuAction> {
    pub action: A,
    pub label: String,
    pub icon: Icon,
    pub danger: bool,
    pub disabled: bool,
}

impl<A> RowItem<A> {
    pub fn new(action: A, label: impl Into<String>, icon: Icon) -> Self {
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

impl<A: Clone> MenuRow<A> {
    pub fn item(action: A, label: impl Into<String>, icon: Icon) -> Self {
        Self::Item(RowItem::new(action, label, icon))
    }

    /// Keyboard navigation skips separators and disabled items.
    pub fn is_selectable(&self) -> bool {
        matches!(self, Self::Item(i) if !i.disabled)
    }

    /// The action Enter would run on this row.
    pub fn action(&self) -> Option<A> {
        match self {
            Self::Item(i) if !i.disabled => Some(i.action.clone()),
            _ => None,
        }
    }
}
