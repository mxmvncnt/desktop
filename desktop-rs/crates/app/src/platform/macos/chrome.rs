//! Window chrome on macOS. The system draws the frame, shadow, corners and
//! traffic lights; we only reserve room for the lights in the toolbar and mark
//! its empty space as the drag region.

use gpui::{
    AnyElement, App, InteractiveElement, Window, WindowButton, WindowButtonLayout,
    WindowControlArea,
};

/// Width the toolbar leaves free for the traffic lights.
const TRAFFIC_LIGHTS_WIDTH: f32 = 78.0;

#[derive(Clone, Copy, Debug)]
pub struct Chrome {
    pub maximized: bool,
}

impl Chrome {
    pub fn detect(window: &Window) -> Self {
        Self { maximized: window.is_maximized() }
    }

    pub fn radius(&self) -> f32 {
        0.0
    }

    /// Extra left padding for the toolbar, to clear the traffic lights.
    /// (Fullscreen hides them, but the gap is harmless.)
    pub fn leading_inset(&self) -> f32 {
        TRAFFIC_LIGHTS_WIDTH
    }

    /// The system draws the window buttons.
    pub fn buttons(&self, _window: &Window, _cx: &App) -> Option<WindowButtonLayout> {
        None
    }
}

pub fn caption_buttons(
    _buttons: &[Option<WindowButton>],
    _maximized: bool,
    _cx: &App,
) -> Option<AnyElement> {
    None
}

/// Drag the window by the toolbar's empty space; the system handles
/// double-click zoom.
pub fn titlebar_drag<E: InteractiveElement>(el: E, _chrome: Chrome) -> E {
    el.window_control_area(WindowControlArea::Drag)
}

pub fn frame(content: AnyElement, _chrome: Chrome, _window: &mut Window, _cx: &App) -> AnyElement {
    content
}
