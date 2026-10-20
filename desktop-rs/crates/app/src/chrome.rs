//! Window chrome for client-side decorations (CSD), modeled on zeron's Linux
//! shell (`.ref/zeron/crates/ui/src/shell.rs`, `render_linux_caption_controls`
//! / `render_linux_resize_borders`).
//!
//! On Wayland most compositors (GNOME in particular) never draw a frame, so
//! the app must provide: a draggable titlebar (our toolbar), caption buttons,
//! resize edges, and, when floating, rounded corners plus a shadow.
//! When the compositor hands back server decorations (KDE/X11 WMs),
//! [`Chrome::detect`] returns `client = false` and none of this renders.

use gd_ui::{Icon, Theme, icon, theme::ink};
use gpui::{
    AnyElement, App, Decorations, InteractiveElement, IntoElement, MouseButton, ParentElement,
    ResizeEdge, StatefulInteractiveElement, Styled, Tiling, Window, WindowButton,
    WindowButtonLayout, div, hsla, millis, prelude::*, px,
};

/// Corner radius of a floating window (zeron: `LINUX_WINDOW_CORNER_RADIUS`).
pub const CORNER_RADIUS: f32 = 10.0;
/// Transparent margin around a floating window that holds the shadow and the
/// outer half of the resize grip.
pub const SHADOW_INSET: f32 = 12.0;
/// Width of the resize strips (they straddle the window edge).
const RESIZE_EDGE: f32 = 8.0;
const RESIZE_CORNER: f32 = 16.0;

#[derive(Clone, Copy, Debug)]
pub struct Chrome {
    /// We draw the decorations ourselves.
    pub client: bool,
    pub tiling: Tiling,
    pub maximized: bool,
}

impl Chrome {
    pub fn detect(window: &Window) -> Self {
        // Snapshot builds can preview the CSD frame headlessly.
        #[cfg(feature = "snapshot")]
        if std::env::var_os("GD_SNAPSHOT_CSD").is_some() {
            return Self {
                client: true,
                tiling: Tiling::default(),
                maximized: false,
            };
        }
        match window.window_decorations() {
            Decorations::Client { tiling } => Self {
                client: true,
                tiling,
                maximized: window.is_maximized(),
            },
            Decorations::Server => Self {
                client: false,
                tiling: Tiling::default(),
                maximized: false,
            },
        }
    }

    /// Floating = CSD with no edge tiled: rounded, shadowed, inset.
    pub fn floating(&self) -> bool {
        self.client && !self.tiling.is_tiled() && !self.maximized
    }

    pub fn radius(&self) -> f32 {
        if self.floating() { CORNER_RADIUS } else { 0.0 }
    }

    pub fn inset(&self) -> f32 {
        if self.floating() { SHADOW_INSET } else { 0.0 }
    }

    /// Caption buttons to draw, honoring the desktop's button layout
    /// (GNOME `button-layout`) and dropping buttons the compositor can't do.
    pub fn buttons(&self, window: &Window, cx: &App) -> Option<WindowButtonLayout> {
        if !self.client {
            return None;
        }
        let layout = cx
            .button_layout()
            .unwrap_or_else(WindowButtonLayout::linux_default);
        let supported = window.window_controls();
        let filter = |side: [Option<WindowButton>; gpui::MAX_BUTTONS_PER_SIDE]| {
            let mut out = [None; gpui::MAX_BUTTONS_PER_SIDE];
            let mut i = 0;
            for b in side.into_iter().flatten() {
                let keep = match b {
                    WindowButton::Minimize => supported.minimize,
                    WindowButton::Maximize => supported.maximize,
                    WindowButton::Close => true,
                };
                if keep {
                    out[i] = Some(b);
                    i += 1;
                }
            }
            out
        };
        Some(WindowButtonLayout {
            left: filter(layout.left),
            right: filter(layout.right),
        })
    }
}

/// One side's caption buttons (minimize / maximize-restore / close).
pub fn caption_buttons(
    buttons: &[Option<WindowButton>],
    maximized: bool,
    cx: &App,
) -> Option<AnyElement> {
    if buttons.iter().all(Option::is_none) {
        return None;
    }
    let theme = Theme::of(cx).clone();
    Some(
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.0))
            .px(px(6.0))
            .children(buttons.iter().flatten().map(|b| {
                let (id, glyph, close): (&'static str, Icon, bool) = match b {
                    WindowButton::Minimize => ("caption-minimize", Icon::WindowMinimize, false),
                    WindowButton::Maximize if maximized => {
                        ("caption-restore", Icon::WindowRestore, false)
                    }
                    WindowButton::Maximize => ("caption-maximize", Icon::WindowMaximize, false),
                    WindowButton::Close => ("caption-close", Icon::Close, true),
                };
                let b = *b;
                let hover_bg = if close {
                    // zeron's close hover: a red plate with a white glyph.
                    hsla(355.0 / 360.0, 0.86, 0.49, 1.0)
                } else {
                    theme.element_hover
                };
                let hover_fg = if close { gpui::white() } else { theme.text };
                div()
                    .id(id)
                    .group(id)
                    .size(px(26.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(7.0))
                    .cursor_pointer()
                    .bg(ink(0.0))
                    .transitions(|t| t.bg(millis(gd_ui::motion::HOVER_MS)))
                    .hover_style(move |s| s.bg(hover_bg))
                    .occlude()
                    // Don't let the titlebar drag handler see this press.
                    .on_mouse_down(MouseButton::Left, |_, window, cx| {
                        window.prevent_default();
                        cx.stop_propagation();
                    })
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        match b {
                            WindowButton::Minimize => window.minimize_window(),
                            WindowButton::Maximize => window.zoom_window(),
                            WindowButton::Close => window.remove_window(),
                        }
                    })
                    .child(
                        icon(glyph)
                            .size(px(16.0))
                            .text_color(theme.text_muted)
                            .group_hover_style(id, move |s| s.text_color(hover_fg)),
                    )
                    .into_any_element()
            }))
            .into_any_element(),
    )
}

/// Make an element behave like a titlebar: drag to move, double-click to
/// maximize/restore, right-click for the compositor's window menu.
pub fn titlebar_drag<E: InteractiveElement>(el: E, chrome: Chrome) -> E {
    if !chrome.client {
        return el;
    }
    el.on_mouse_down(MouseButton::Left, |e, window, _| {
        if e.click_count >= 2 {
            window.zoom_window();
        } else {
            window.start_window_move();
        }
    })
    .on_mouse_down(MouseButton::Right, |e, window, _| {
        window.show_window_menu(e.position)
    })
}

/// Wrap the app content in the CSD frame: transparent shadow margin, rounded
/// hairline-bordered window, and resize strips on every free edge.
pub fn frame(content: AnyElement, chrome: Chrome, window: &mut Window, cx: &App) -> AnyElement {
    if !chrome.client {
        return content;
    }
    let inset = chrome.inset();
    window.set_client_inset(px(inset));
    let theme = Theme::of(cx);
    let radius = px(chrome.radius());
    let t = chrome.tiling;
    let resizable = window.is_resizable() && !chrome.maximized;

    let window_el = div()
        .size_full()
        .overflow_hidden()
        .rounded(radius)
        .when(chrome.floating(), |d| {
            d.border_1().border_color(theme.border_strong).shadow(vec![
                gpui::BoxShadow::new(px(0.0), px(4.0), hsla(0.0, 0.0, 0.0, 0.45))
                    .blur_radius(px(inset * 0.9)),
            ])
        })
        .child(content);

    let mut root = div()
        .id("window-frame")
        .size_full()
        .relative()
        .bg(gpui::transparent_black())
        .p(px(inset))
        .child(window_el);

    if resizable {
        let (top, bottom, left, right) = (!t.top, !t.bottom, !t.left, !t.right);
        // Strips straddle the visible edge: half in the shadow margin, half
        // over the window's outermost pixels.
        let o = (inset - RESIZE_EDGE / 2.0).max(0.0);
        let c = RESIZE_CORNER;
        let strip = |edge: ResizeEdge| {
            div()
                .absolute()
                .occlude()
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    window.start_window_resize(edge);
                })
        };
        let px_ = |v: f32| px(v);
        let e = RESIZE_EDGE;
        let mut edges: Vec<AnyElement> = Vec::new();
        if top {
            edges.push(strip(ResizeEdge::Top).top(px_(o)).left(px_(o + c)).right(px_(o + c)).h(px_(e)).cursor_ns_resize().into_any_element());
        }
        if bottom {
            edges.push(strip(ResizeEdge::Bottom).bottom(px_(o)).left(px_(o + c)).right(px_(o + c)).h(px_(e)).cursor_ns_resize().into_any_element());
        }
        if left {
            edges.push(strip(ResizeEdge::Left).left(px_(o)).top(px_(o + c)).bottom(px_(o + c)).w(px_(e)).cursor_ew_resize().into_any_element());
        }
        if right {
            edges.push(strip(ResizeEdge::Right).right(px_(o)).top(px_(o + c)).bottom(px_(o + c)).w(px_(e)).cursor_ew_resize().into_any_element());
        }
        if top && left {
            edges.push(strip(ResizeEdge::TopLeft).top(px_(o)).left(px_(o)).size(px_(c)).cursor_nwse_resize().into_any_element());
        }
        if top && right {
            edges.push(strip(ResizeEdge::TopRight).top(px_(o)).right(px_(o)).size(px_(c)).cursor_nesw_resize().into_any_element());
        }
        if bottom && left {
            edges.push(strip(ResizeEdge::BottomLeft).bottom(px_(o)).left(px_(o)).size(px_(c)).cursor_nesw_resize().into_any_element());
        }
        if bottom && right {
            edges.push(strip(ResizeEdge::BottomRight).bottom(px_(o)).right(px_(o)).size(px_(c)).cursor_nwse_resize().into_any_element());
        }
        root = root.children(edges);
    }
    root.into_any_element()
}
