//! `GD_DEBUG_FRAMES=1`: once per second, log how many times the root view
//! rendered and how many pointer-move events arrived. Idle should read
//! `renders=0 moves=0`. Renders well above moves mean something is requesting
//! frames on its own (a repeating animation, a transition that never settles,
//! or a render path that notifies). Zero cost when the env var is unset.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::Instant;

static RENDERS: AtomicU64 = AtomicU64::new(0);
static MOVES: AtomicU64 = AtomicU64::new(0);
static LAST: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);

fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("GD_DEBUG_FRAMES").is_some())
}

pub fn on_input() {
    if enabled() {
        MOVES.fetch_add(1, Relaxed);
    }
}

pub fn on_render() {
    if !enabled() {
        return;
    }
    RENDERS.fetch_add(1, Relaxed);
    let mut last = LAST.lock().unwrap();
    let now = Instant::now();
    match *last {
        Some(t) if now.duration_since(t).as_secs_f32() < 1.0 => {}
        _ => {
            let r = RENDERS.swap(0, Relaxed);
            let m = MOVES.swap(0, Relaxed);
            log::info!("frames: renders={r} moves={m} (last ~1s)");
            *last = Some(now);
        }
    }
}
