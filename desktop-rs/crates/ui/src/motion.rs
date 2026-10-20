//! Motion catalog, ported from zeron's `motion.rs`:
//!
//! - `fade-in`    500ms cubic-bezier(0.16,1,0.3,1), translateY 4→0 (entrances)
//! - `fade-quick` 150ms
//! - `menu-in`    140ms translateY −2→0 + fade (popovers)
//! - `dialog-in`  180ms fade + translateY 6→0
//! - hover washes: 120ms bg transition (via GPUI-CE `.transitions(..)`)
//!
//! Every `with_animation` element honors `App::reduce_motion` automatically.
//! Animations are one-shot: nothing here schedules frames once settled, which
//! matters for idle CPU (performance is a primary goal — see docs/PERFORMANCE.md).

use std::time::Duration;

use gpui::{Animation, AnimationExt, ElementId, IntoElement, Styled, px};

pub const HOVER_MS: u64 = 120;
pub const FADE_IN: Duration = Duration::from_millis(500);
pub const FADE_QUICK: Duration = Duration::from_millis(150);
pub const MENU_IN: Duration = Duration::from_millis(140);
pub const DIALOG_IN: Duration = Duration::from_millis(180);

/// CSS `cubic-bezier(x1, y1, x2, y2)` evaluated exactly (Newton + bisection).
#[derive(Clone, Copy)]
pub struct CubicBezier(pub f32, pub f32, pub f32, pub f32);

impl CubicBezier {
    /// zeron's signature ease-out: `cubic-bezier(0.16, 1, 0.3, 1)`.
    pub const EXPO_OUT: Self = Self(0.16, 1.0, 0.3, 1.0);

    fn sample(a1: f32, a2: f32, t: f32) -> f32 {
        let u = 1.0 - t;
        3.0 * u * u * t * a1 + 3.0 * u * t * t * a2 + t * t * t
    }

    pub fn eval(self, x: f32) -> f32 {
        let Self(x1, y1, x2, y2) = self;
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }
        let (mut lo, mut hi, mut t) = (0.0f32, 1.0f32, x);
        for _ in 0..12 {
            let cx = Self::sample(x1, x2, t) - x;
            if cx.abs() < 1e-5 {
                break;
            }
            if cx > 0.0 {
                hi = t;
            } else {
                lo = t;
            }
            t = (lo + hi) * 0.5;
        }
        Self::sample(y1, y2, t)
    }

    pub fn easing(self) -> impl Fn(f32) -> f32 + 'static {
        move |x| self.eval(x)
    }
}

/// Entrance: fade + rise 4px over 500ms with the expo-out curve.
pub fn fade_in<E: IntoElement + Styled + 'static>(
    id: impl Into<ElementId>,
    el: E,
) -> impl IntoElement {
    el.with_animation(
        id,
        Animation::new(FADE_IN).with_easing(CubicBezier::EXPO_OUT.easing()),
        |el, t| el.opacity(t).relative().top(px(4.0 * (1.0 - t))),
    )
}

/// Quick fade (150ms) for content swaps (diff reloads, tab switches).
pub fn fade_quick<E: IntoElement + Styled + 'static>(
    id: impl Into<ElementId>,
    el: E,
) -> impl IntoElement {
    el.with_animation(
        id,
        Animation::new(FADE_QUICK).with_easing(CubicBezier::EXPO_OUT.easing()),
        |el, t| el.opacity(t),
    )
}

/// Popover/menu entrance: fade + drop from −2px over 140ms.
pub fn menu_in<E: IntoElement + Styled + 'static>(
    id: impl Into<ElementId>,
    el: E,
) -> impl IntoElement {
    el.with_animation(
        id,
        Animation::new(MENU_IN).with_easing(CubicBezier::EXPO_OUT.easing()),
        |el, t| el.opacity(t).relative().top(px(-2.0 * (1.0 - t))),
    )
}

/// Dialog entrance: fade + rise 6px over 180ms.
pub fn dialog_in<E: IntoElement + Styled + 'static>(
    id: impl Into<ElementId>,
    el: E,
) -> impl IntoElement {
    el.with_animation(
        id,
        Animation::new(DIALOG_IN).with_easing(CubicBezier::EXPO_OUT.easing()),
        |el, t| el.opacity(t).relative().top(px(6.0 * (1.0 - t))),
    )
}
