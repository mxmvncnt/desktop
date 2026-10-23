//! Rubber-band overscroll for a scrollable child.
//!
//! GPUI's scroll containers clamp at the edges and never report that a wheel
//! event was swallowed, so `Overscroll` asks a [`ScrollProbe`] whether the list
//! sits at an edge and, if the (precise, i.e. trackpad) delta pushes further,
//! shifts the child by a damped offset. When the wheel goes quiet the offset
//! springs back with the expo-out curve. Line deltas from a notched mouse wheel
//! never bounce. The spring-back is one-shot: it requests frames only while it
//! runs, so a settled view renders nothing extra.

use std::time::{Duration, Instant};

use gd_ui::motion::CubicBezier;
use gpui::{
    AnyElement, App, Context, ElementId, InteractiveElement, IntoElement, ListState, ParentElement, RenderOnce,
    ScrollWheelEvent, Styled, Task, TouchPhase, UniformListScrollHandle, Window, div, px,
};

/// Furthest the content can be pulled past an edge.
const MAX_PULL: f32 = 64.0;
/// Fraction of the finger delta that turns into pull (before edge resistance).
const GAIN: f32 = 0.55;
/// Wheel silence that counts as "gesture ended" when the platform sends no phase.
const QUIET: Duration = Duration::from_millis(90);
const SPRING_BACK: Duration = Duration::from_millis(380);

/// Where a scrollable currently is, so `Overscroll` can tell if it is at an edge.
#[derive(Clone)]
pub enum ScrollProbe {
    Uniform(UniformListScrollHandle),
    List(ListState),
}

impl ScrollProbe {
    /// `(scrolled distance >= 0, max scrollable distance)` in pixels.
    fn extent(&self) -> (f32, f32) {
        match self {
            Self::Uniform(h) => {
                let h = h.0.borrow();
                (-f32::from(h.base_handle.offset().y), f32::from(h.base_handle.max_offset().y))
            }
            Self::List(s) => (-f32::from(s.scroll_px_offset_for_scrollbar().y), f32::from(s.max_offset_for_scrollbar().y)),
        }
    }
}

impl From<&UniformListScrollHandle> for ScrollProbe {
    fn from(h: &UniformListScrollHandle) -> Self {
        Self::Uniform(h.clone())
    }
}

impl From<ListState> for ScrollProbe {
    fn from(s: ListState) -> Self {
        Self::List(s)
    }
}

#[derive(IntoElement)]
pub struct Overscroll {
    id: ElementId,
    probe: ScrollProbe,
    child: AnyElement,
}

impl Overscroll {
    pub fn new(id: impl Into<ElementId>, probe: impl Into<ScrollProbe>, child: impl IntoElement) -> Self {
        Self { id: id.into(), probe: probe.into(), child: child.into_any_element() }
    }
}

impl RenderOnce for Overscroll {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id, cx, |_, _| Bounce::new());
        let offset = state.update(cx, |b, _| b.current());
        if state.read(cx).release.is_some() {
            window.request_animation_frame();
        }
        let probe = self.probe;
        let handle = state.clone();
        let mut inner = div().size_full().flex().flex_col().min_h_0();
        if offset != 0.0 {
            inner = inner.relative().top(px(offset));
        }
        div()
            .size_full()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .on_scroll_wheel(move |e, window, cx| {
                if let Some(pull) = Pull::from_event(e, window) {
                    let (scrolled, max) = probe.extent();
                    handle.update(cx, |b, cx| b.push(pull, scrolled, max, cx));
                }
            })
            .child(inner.child(self.child))
    }
}

/// A precise vertical wheel delta, in pixels (positive = content moves down).
struct Pull {
    dy: f32,
    ended: bool,
}

impl Pull {
    fn from_event(e: &ScrollWheelEvent, window: &Window) -> Option<Self> {
        if !e.delta.precise() {
            return None;
        }
        let d = e.delta.pixel_delta(window.line_height());
        let (dx, dy) = (f32::from(d.x), f32::from(d.y));
        (dy.abs() > dx.abs()).then_some(Self {
            dy,
            ended: matches!(e.touch_phase, TouchPhase::Ended | TouchPhase::Cancelled),
        })
    }
}

struct Bounce {
    /// Current pull in pixels: positive past the top, negative past the bottom.
    offset: f32,
    /// Spring-back start time and starting offset.
    release: Option<(Instant, f32)>,
    quiet_timer: Option<Task<()>>,
}

impl Bounce {
    fn new() -> Self {
        // GD_OVERSCROLL_PREVIEW=<px> pins a pull for snapshots and manual checks.
        let offset = std::env::var("GD_OVERSCROLL_PREVIEW").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        Self { offset, release: None, quiet_timer: None }
    }

    /// The offset to draw now; settles (and stops asking for frames) when done.
    fn current(&mut self) -> f32 {
        let Some((start, from)) = self.release else { return self.offset };
        let t = start.elapsed().as_secs_f32() / SPRING_BACK.as_secs_f32();
        if t >= 1.0 {
            self.offset = 0.0;
            self.release = None;
            return 0.0;
        }
        self.offset = from * (1.0 - CubicBezier::EXPO_OUT.eval(t));
        self.offset
    }

    fn push(&mut self, pull: Pull, scrolled: f32, max: f32, cx: &mut Context<Self>) {
        let at_top = scrolled <= 0.5;
        let at_bottom = scrolled >= max - 0.5;
        let pushing_out = (pull.dy > 0.0 && at_top) || (pull.dy < 0.0 && at_bottom);
        let engaged = self.offset != 0.0;
        if !pushing_out && !engaged {
            return;
        }
        // Interrupting a spring-back resumes from the last drawn offset.
        self.release = None;
        let same_way = self.offset == 0.0 || (self.offset > 0.0) == (pull.dy > 0.0);
        let resistance = if same_way { (1.0 - self.offset.abs() / MAX_PULL).max(0.0).powi(2) } else { 1.0 };
        let next = self.offset + pull.dy * GAIN * resistance;
        // Pulling back across zero stops at zero instead of flipping edges.
        self.offset = if !same_way && next.signum() != self.offset.signum() { 0.0 } else { next.clamp(-MAX_PULL, MAX_PULL) };
        if pull.ended {
            self.begin_release(cx);
        } else {
            self.arm_quiet_timer(cx);
        }
        cx.notify();
    }

    fn arm_quiet_timer(&mut self, cx: &mut Context<Self>) {
        self.quiet_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(QUIET).await;
            this.update(cx, |b, cx| b.begin_release(cx)).ok();
        }));
    }

    fn begin_release(&mut self, cx: &mut Context<Self>) {
        self.quiet_timer = None;
        if self.offset == 0.0 {
            return;
        }
        if cx.reduce_motion() {
            self.offset = 0.0;
        } else {
            self.release = Some((Instant::now(), self.offset));
        }
        cx.notify();
    }
}
