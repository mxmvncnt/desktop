//! Zeron-style overlays: [`ContextMenu`] and [`Dialog`].
//!
//! Both are `RenderOnce` and render a full-window `deferred` layer (backdrop +
//! card), so they sit above everything. They are stateless: the owner decides
//! when they exist (render them only while open) and handles selection /
//! keyboard focus. Entrances are one-shot (`menu_in` / `dialog_in`).

use std::rc::Rc;

use gpui::{
    App, ElementId, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Pixels, Point, RenderOnce, SharedString, Styled, Window, deferred, div, prelude::*, px,
};

use crate::assets::Icon;
use crate::components::{Button, ButtonStyle, icon, overlay_card, pressable};
use crate::motion;
use crate::theme::{Theme, ink, scrim};

type Callback = Rc<dyn Fn(&mut Window, &mut App)>;

/// One selectable row of a [`ContextMenu`].
pub struct MenuItem {
    pub label: SharedString,
    pub icon: Option<Icon>,
    pub danger: bool,
    pub disabled: bool,
    pub on_select: Option<Callback>,
}

impl MenuItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self { label: label.into(), icon: None, danger: false, disabled: false, on_select: None }
    }
    pub fn icon(mut self, i: Icon) -> Self {
        self.icon = Some(i);
        self
    }
    /// Destructive item: danger-colored label and icon.
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn on_select(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

pub enum MenuEntry {
    Item(MenuItem),
    Separator,
}

const MENU_WIDTH: f32 = 236.0;
const MENU_ITEM_H: f32 = 28.0;
const MENU_PAD: f32 = 4.0;
const SEPARATOR_H: f32 = 9.0;

/// Floating menu at a window position (right-click). The owner highlights
/// `selected` (an index into the entries) for keyboard navigation.
#[derive(IntoElement)]
pub struct ContextMenu {
    id: ElementId,
    position: Point<Pixels>,
    entries: Vec<MenuEntry>,
    selected: Option<usize>,
    on_dismiss: Option<Callback>,
}

impl ContextMenu {
    pub fn new(id: impl Into<ElementId>, position: Point<Pixels>) -> Self {
        Self { id: id.into(), position, entries: Vec::new(), selected: None, on_dismiss: None }
    }
    pub fn item(mut self, item: MenuItem) -> Self {
        self.entries.push(MenuEntry::Item(item));
        self
    }
    pub fn separator(mut self) -> Self {
        self.entries.push(MenuEntry::Separator);
        self
    }
    pub fn entries(mut self, entries: impl IntoIterator<Item = MenuEntry>) -> Self {
        self.entries.extend(entries);
        self
    }
    pub fn selected(mut self, ix: Option<usize>) -> Self {
        self.selected = ix;
        self
    }
    /// Called when the backdrop is clicked (left or right).
    pub fn on_dismiss(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ContextMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        // Keep the card inside the window (flip up / shift left near edges).
        // `position` is in window coordinates; under client-side decorations
        // the content sits inside a transparent shadow inset, so translate.
        let inset = window.client_inset().unwrap_or(px(0.0));
        let position = gpui::point(self.position.x - inset, self.position.y - inset);
        let mut view = window.viewport_size();
        view.width -= inset * 2.0;
        view.height -= inset * 2.0;
        let height: f32 = MENU_PAD * 2.0
            + self
                .entries
                .iter()
                .map(|e| match e {
                    MenuEntry::Item(_) => MENU_ITEM_H,
                    MenuEntry::Separator => SEPARATOR_H,
                })
                .sum::<f32>();
        let x = position.x.min(view.width - px(MENU_WIDTH + 8.0)).max(px(8.0));
        let y = if position.y + px(height + 8.0) > view.height {
            (position.y - px(height)).max(px(8.0))
        } else {
            position.y
        };

        let mut card = overlay_card(cx)
            .id(self.id.clone())
            .occlude()
            .w(px(MENU_WIDTH))
            .p(px(MENU_PAD))
            .flex()
            .flex_col();
        for (ix, entry) in self.entries.into_iter().enumerate() {
            card = card.child(match entry {
                MenuEntry::Separator => div()
                    .h(px(SEPARATOR_H))
                    .flex()
                    .items_center()
                    .child(div().h(px(1.0)).w_full().bg(theme.border))
                    .into_any_element(),
                MenuEntry::Item(item) => {
                    let fg = if item.danger { theme.danger } else { theme.text };
                    let icon_fg = if item.danger { theme.danger } else { theme.text_muted };
                    let selected = self.selected == Some(ix) && !item.disabled;
                    let mut row = pressable(("menu-item", ix), selected, cx)
                        .flex()
                        .items_center()
                        .gap(px(9.0))
                        .h(px(MENU_ITEM_H))
                        .px(px(8.0))
                        .rounded(px(6.0))
                        .text_size(px(13.0))
                        .text_color(fg)
                        .child(
                            div().flex_none().w(px(15.0)).children(
                                item.icon.map(|i| icon(i).size(px(15.0)).text_color(icon_fg)),
                            ),
                        )
                        .child(div().flex_1().min_w_0().truncate().child(item.label));
                    if item.disabled {
                        row = row.opacity(0.4).cursor_default();
                    } else if let Some(f) = item.on_select {
                        row = row.on_click(move |_, w, cx| f(w, cx));
                    }
                    row.into_any_element()
                }
            });
        }

        let dismiss = self.on_dismiss;
        let mut backdrop = div().id("menu-backdrop").absolute().inset_0();
        if let Some(f) = dismiss {
            let g = f.clone();
            backdrop = backdrop
                .on_mouse_down(MouseButton::Left, move |_, w, cx| f(w, cx))
                .on_mouse_down(MouseButton::Right, move |_, w, cx| g(w, cx));
        }
        deferred(
            div().absolute().inset_0().child(backdrop).child(
                div()
                    .absolute()
                    .left(x)
                    .top(y)
                    .child(motion::menu_in("context-menu-in", card)),
            ),
        )
    }
}

/// Modal confirmation dialog: scrim, centered card, title, body, and
/// Cancel + confirm buttons (confirm is primary, or red with [`Dialog::danger`]).
#[derive(IntoElement)]
pub struct Dialog {
    id: ElementId,
    title: SharedString,
    body: Option<SharedString>,
    detail: Option<SharedString>,
    content: Option<gpui::AnyElement>,
    confirm_label: SharedString,
    cancel_label: SharedString,
    danger: bool,
    on_confirm: Option<Callback>,
    on_cancel: Option<Callback>,
}

impl Dialog {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            body: None,
            detail: None,
            content: None,
            confirm_label: "OK".into(),
            cancel_label: "Cancel".into(),
            danger: false,
            on_confirm: None,
            on_cancel: None,
        }
    }
    pub fn body(mut self, b: impl Into<SharedString>) -> Self {
        self.body = Some(b.into());
        self
    }
    /// Smaller, muted secondary text under the body.
    pub fn detail(mut self, d: impl Into<SharedString>) -> Self {
        self.detail = Some(d.into());
        self
    }
    /// Custom body content (inputs, option rows) between the text and the buttons.
    pub fn content(mut self, c: impl IntoElement) -> Self {
        self.content = Some(c.into_any_element());
        self
    }
    pub fn confirm_label(mut self, l: impl Into<SharedString>) -> Self {
        self.confirm_label = l.into();
        self
    }
    pub fn cancel_label(mut self, l: impl Into<SharedString>) -> Self {
        self.cancel_label = l.into();
        self
    }
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }
    pub fn on_confirm(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_confirm = Some(Rc::new(f));
        self
    }
    /// Called for Cancel, a backdrop click, and (by the owner) Esc.
    pub fn on_cancel(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_cancel = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Dialog {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let cancel = self.on_cancel;
        let confirm = self.on_confirm;

        let mut cancel_btn = Button::new("dialog-cancel").label(self.cancel_label);
        if let Some(f) = cancel.clone() {
            cancel_btn = cancel_btn.on_click(move |_, w, cx| f(w, cx));
        }
        let mut confirm_btn = Button::new("dialog-confirm")
            .label(self.confirm_label)
            .style(if self.danger { ButtonStyle::Danger } else { ButtonStyle::Primary });
        if let Some(f) = confirm {
            confirm_btn = confirm_btn.on_click(move |_, w, cx| f(w, cx));
        }

        let card = overlay_card(cx)
            .id(self.id)
            .occlude()
            .w(px(440.0))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .px(px(20.0))
                    .pt(px(18.0))
                    .pb(px(16.0))
                    .child(
                        div()
                            .text_size(px(15.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(self.title),
                    )
                    .children(self.body.map(|b| {
                        div().text_size(px(13.0)).text_color(theme.text_muted).child(b)
                    }))
                    .children(self.detail.map(|d| {
                        div().text_size(px(12.0)).text_color(theme.text_faint).child(d)
                    }))
                    .children(self.content),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_t_1()
                    .border_color(theme.border)
                    .bg(ink(0.02))
                    .child(cancel_btn)
                    .child(confirm_btn),
            );

        let mut backdrop = div().id("dialog-backdrop").absolute().inset_0().bg(scrim(0.5));
        if let Some(f) = cancel {
            backdrop = backdrop.on_mouse_down(MouseButton::Left, move |_, w, cx| f(w, cx));
        }
        deferred(
            div()
                .absolute()
                .inset_0()
                .child(motion::fade_quick("dialog-scrim-in", backdrop))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(motion::dialog_in("dialog-in", card)),
                ),
        )
    }
}
