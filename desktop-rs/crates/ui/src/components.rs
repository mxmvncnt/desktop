//! Reusable components in zeron's visual language: borderless ghost controls
//! with soft ink hover washes that *transition* (never snap), hairline
//! dividers, a near-white solid plate for the primary action.

use gpui::{
    AnyElement, App, ClickEvent, Div, ElementId, Hsla, IntoElement, ParentElement, RenderOnce,
    SharedString, Stateful, Styled, Svg, Window, div, millis, prelude::*, px, svg,
};

use gpui::AnimationExt as _;

use crate::assets::Icon;
use crate::motion::HOVER_MS;
use crate::theme::{CONTROL_RADIUS, Theme, ink};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

pub fn icon(icon: Icon) -> Svg {
    svg().path(icon.path()).size(px(16.0)).flex_none()
}

/// Horizontal hairline divider.
pub fn divider(cx: &App) -> Div {
    div().h(px(1.0)).w_full().flex_none().bg(Theme::of(cx).border)
}

/// Vertical hairline divider.
pub fn vdivider(cx: &App) -> Div {
    div().w(px(1.0)).h_full().flex_none().bg(Theme::of(cx).border)
}

/// An interactive surface with zeron's hover/active ink washes, transitioned.
/// Base building block for rows and ghost buttons.
pub fn pressable(id: impl Into<ElementId>, selected: bool, cx: &App) -> Stateful<Div> {
    let theme = Theme::of(cx);
    let rest = if selected {
        theme.element_active
    } else {
        ink(0.0)
    };
    let hover = if selected {
        theme.element_active
    } else {
        theme.element_hover
    };
    let active = theme.element_active;
    div()
        .id(id)
        .bg(rest)
        .transitions(|t| t.bg(millis(HOVER_MS)))
        .hover_style(move |s| s.bg(hover))
        .active_style(move |s| s.bg(active))
        .cursor_pointer()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonStyle {
    /// Near-white solid plate, dark label. The one primary action per view.
    Primary,
    /// Translucent raised pill.
    Secondary,
    /// No plate until hovered.
    Ghost,
    /// Red plate for destructive confirmations.
    Danger,
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    icon: Option<Icon>,
    style: ButtonStyle,
    disabled: bool,
    full_width: bool,
    compact: bool,
    on_click: Option<ClickHandler>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            icon: None,
            style: ButtonStyle::Secondary,
            disabled: false,
            full_width: false,
            compact: false,
            on_click: None,
        }
    }
    pub fn label(mut self, l: impl Into<SharedString>) -> Self {
        self.label = Some(l.into());
        self
    }
    pub fn icon(mut self, i: Icon) -> Self {
        self.icon = Some(i);
        self
    }
    pub fn style(mut self, s: ButtonStyle) -> Self {
        self.style = s;
        self
    }
    pub fn primary(self) -> Self {
        self.style(ButtonStyle::Primary)
    }
    pub fn ghost(self) -> Self {
        self.style(ButtonStyle::Ghost)
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }
    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx);
        let (bg, hover, fg): (Hsla, Hsla, Hsla) = match self.style {
            ButtonStyle::Primary => (theme.solid, theme.text_muted, theme.on_solid),
            ButtonStyle::Secondary => (ink(0.06), ink(0.10), theme.text),
            ButtonStyle::Ghost => (ink(0.0), theme.element_hover, theme.text_muted),
            ButtonStyle::Danger => (theme.danger_strong, theme.danger, theme.on_accent),
        };
        let hover = if self.style == ButtonStyle::Primary {
            gpui::hsla(0.0, 0.0, if crate::theme::is_light() { 0.0 } else { 1.0 }, 1.0)
        } else {
            hover
        };
        let hover_fg = if self.style == ButtonStyle::Ghost {
            theme.text
        } else {
            fg
        };
        let mut el = div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(px(6.0))
            .h(px(if self.compact { 26.0 } else { 30.0 }))
            .px(px(if self.label.is_some() { 12.0 } else { 7.0 }))
            .rounded(px(CONTROL_RADIUS))
            .text_size(px(13.0))
            .font_weight(gpui::FontWeight::MEDIUM)
            .bg(bg)
            .text_color(fg)
            .when(self.full_width, |d| d.w_full())
            .when_some(self.icon, |d, i| d.child(icon(i).size(px(15.0)).text_color(fg)))
            .when_some(self.label, |d, l| d.child(l));
        if self.disabled {
            el = el.opacity(0.4);
        } else {
            el = el
                .cursor_pointer()
                .transitions(|t| t.bg(millis(HOVER_MS)).text_color(millis(HOVER_MS)))
                .hover_style(move |s| s.bg(hover).text_color(hover_fg));
            if let Some(f) = self.on_click {
                el = el.on_click(f);
            }
        }
        el
    }
}

/// Square icon-only ghost button (toolbar/header affordances).
pub fn icon_button(id: impl Into<ElementId>, i: Icon) -> Button {
    Button::new(id).icon(i).ghost()
}

/// The checkbox used in the changes list.
#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    state: CheckState,
    on_click: Option<ClickHandler>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CheckState {
    Checked,
    Unchecked,
    Mixed,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>, state: CheckState) -> Self {
        Self {
            id: id.into(),
            state,
            on_click: None,
        }
    }
    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Checkbox {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx);
        let on = self.state != CheckState::Unchecked;
        let mut el = div()
            .id(self.id)
            .size(px(15.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.0))
            .border_1()
            .border_color(if on { theme.solid } else { theme.border_strong })
            .bg(if on { theme.solid } else { ink(0.0) })
            .transitions(|t| t.bg(millis(HOVER_MS)).border_color(millis(HOVER_MS)))
            .hover_style({
                let c = theme.text_muted;
                move |s| s.border_color(c)
            })
            .cursor_pointer()
            .map(|d| match self.state {
                CheckState::Checked => {
                    d.child(icon(Icon::Check).size(px(12.0)).text_color(theme.on_solid))
                }
                CheckState::Mixed => d.child(
                    div()
                        .w(px(7.0))
                        .h(px(1.5))
                        .rounded(px(1.0))
                        .bg(theme.on_solid),
                ),
                CheckState::Unchecked => d,
            });
        if let Some(f) = self.on_click {
            el = el.on_click(move |e, w, cx| {
                cx.stop_propagation();
                f(e, w, cx)
            });
        }
        el
    }
}

/// Segmented tab bar ("Changes" | "History"). Selected tab gets a raised
/// plate; the plate color transitions so switching feels like it slides.
#[derive(IntoElement)]
pub struct Tabs {
    id: ElementId,
    items: Vec<(SharedString, Option<SharedString>)>,
    selected: usize,
    on_select: Option<std::rc::Rc<dyn Fn(usize, &mut Window, &mut App)>>,
}

impl Tabs {
    pub fn new(id: impl Into<ElementId>, selected: usize) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            selected,
            on_select: None,
        }
    }
    pub fn tab(mut self, label: impl Into<SharedString>, badge: Option<SharedString>) -> Self {
        self.items.push((label.into(), badge));
        self
    }
    pub fn on_select(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for Tabs {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        div()
            .id(self.id)
            .flex()
            .gap(px(2.0))
            .p(px(3.0))
            .rounded(px(CONTROL_RADIUS + 3.0))
            .bg(ink(0.03))
            .children(self.items.into_iter().enumerate().map(|(ix, (label, badge))| {
                let selected = ix == self.selected;
                let on_select = self.on_select.clone();
                div()
                    .id(ix)
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(6.0))
                    .h(px(26.0))
                    .rounded(px(CONTROL_RADIUS))
                    .text_size(px(13.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .cursor_pointer()
                    .bg(if selected { ink(0.09) } else { ink(0.0) })
                    .text_color(if selected { theme.text } else { theme.text_faint })
                    .transitions(|t| t.bg(millis(180)).text_color(millis(180)))
                    .hover_style({
                        let c = theme.text;
                        move |s| s.text_color(c)
                    })
                    .child(label)
                    .when_some(badge, |d, b| {
                        d.child(
                            div()
                                .px(px(6.0))
                                .rounded_full()
                                .bg(ink(0.08))
                                .text_size(px(11.0))
                                .text_color(theme.text_muted)
                                .child(b),
                        )
                    })
                    .when_some(on_select, |d, f| d.on_click(move |_, w, cx| f(ix, w, cx)))
            }))
    }
}

/// Toolbar dropdown in Desktop's layout ("Current repository ▾"), drawn
/// zeron-style: small muted caption over a medium title, ghost hover wash.
#[derive(IntoElement)]
pub struct ToolbarButton {
    id: ElementId,
    icon: Icon,
    caption: SharedString,
    title: SharedString,
    chevron: bool,
    open: bool,
    busy: bool,
    width: Option<f32>,
    on_click: Option<ClickHandler>,
}

impl ToolbarButton {
    pub fn new(
        id: impl Into<ElementId>,
        icon: Icon,
        caption: impl Into<SharedString>,
        title: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            icon,
            caption: caption.into(),
            title: title.into(),
            chevron: false,
            open: false,
            busy: false,
            width: None,
            on_click: None,
        }
    }
    pub fn chevron(mut self) -> Self {
        self.chevron = true;
        self
    }
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for ToolbarButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let glyph = icon(self.icon).text_color(theme.text_muted);
        let icon_el: AnyElement = if self.busy {
            glyph
                .with_animation(
                    "busy",
                    gpui::Animation::new(std::time::Duration::from_millis(900))
                        .repeat()
                        .with_easing(gpui::pulsating_between(0.35, 1.0)),
                    |d, t| d.opacity(t),
                )
                .into_any_element()
        } else {
            glyph.into_any_element()
        };
        let mut el = pressable(self.id, self.open, cx)
            .flex()
            .items_center()
            .gap(px(10.0))
            .h_full()
            .px(px(14.0))
            .rounded(px(CONTROL_RADIUS + 2.0))
            .when_some(self.width, |d, w| d.w(px(w)))
            .when(self.width.is_none(), |d| d.min_w(px(180.0)))
            .child(icon_el)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme.text_faint)
                            .line_height(px(14.0))
                            .child(self.caption),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .line_height(px(17.0))
                            .truncate()
                            .child(self.title),
                    ),
            )
            .when(self.chevron, |d| {
                d.child(
                    icon(if self.open {
                        Icon::ChevronUp
                    } else {
                        Icon::ChevronDown
                    })
                    .size(px(14.0))
                    .text_color(theme.text_faint),
                )
            });
        if let Some(f) = self.on_click {
            el = el.on_click(f);
        }
        el
    }
}

/// A small pill badge (counts, ahead/behind).
pub fn badge(text: impl Into<SharedString>, cx: &App) -> Div {
    let theme = Theme::of(cx);
    div()
        .flex_none()
        .px(px(6.0))
        .h(px(18.0))
        .flex()
        .items_center()
        .rounded_full()
        .bg(ink(0.08))
        .text_size(px(11.0))
        .text_color(theme.text_muted)
        .child(text.into())
}

/// The floating card used by popovers, menus and dialogs.
pub fn overlay_card(cx: &App) -> Div {
    let theme = Theme::of(cx);
    div()
        .bg(theme.surface_overlay)
        .border_1()
        .border_color(theme.border_strong)
        .rounded(px(crate::theme::PANEL_RADIUS + 2.0))
        .shadow_lg()
        .overflow_hidden()
}
