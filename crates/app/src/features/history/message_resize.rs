//! Height of the commit message area in the history header: clamping and
//! drag updates (the handle is `message_handle.rs`).

use gpui::Context;

use crate::workspace::Workspace;

/// Height when the message is short enough to fit, and the user limits.
pub const MSG_DEFAULT_MAX: f32 = 120.0;
pub const MSG_MIN: f32 = 40.0;
pub const MSG_MAX: f32 = 520.0;

#[derive(Default)]
pub struct MessageResize {
    /// `None` = fit content up to [`MSG_DEFAULT_MAX`].
    pub height: Option<f32>,
    /// Pointer y and area height when the current drag started.
    anchor: Option<(f32, f32)>,
}

impl MessageResize {
    pub fn end_drag(&mut self) {
        self.anchor = None;
    }
}

impl Workspace {
    /// Live update while dragging the handle; `y` is the pointer's window y.
    pub fn drag_commit_message_to(&mut self, y: f32, shown_height: f32, cx: &mut Context<Self>) {
        let (y0, h0) = *self.commit_msg.anchor.get_or_insert((y, shown_height));
        let h = (h0 + y - y0).clamp(MSG_MIN, MSG_MAX).round();
        if self.commit_msg.height != Some(h) {
            self.commit_msg.height = Some(h);
            cx.notify();
        }
    }

    pub fn reset_commit_message_height(&mut self, cx: &mut Context<Self>) {
        self.commit_msg.height = None;
        cx.notify();
    }
}
