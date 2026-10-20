//! Headless screenshot mode (feature `snapshot`).
//!
//! Renders the real `Workspace` offscreen through GPUI-CE's headless WGPU
//! renderer and writes a PNG. Used by humans and AI sessions to check visuals
//! without a display. Git work runs on the deterministic test dispatcher, so
//! `run_until_parked` waits for status/diff/history loads to finish.
//!
//! ```sh
//! source env.sh
//! GD_CONFIG_DIR=$PWD/.tmp/config cargo run --features snapshot -- \
//!     --snapshot .tmp/shot.png .tmp/testrepo history
//! ```
//! Views: `changes` (default), `history`, `repos`, `branches`, `new-branch`,
//! `clone`, `welcome`, `watch` (headless file-watcher check, no PNG), `icons` (icon-rendering sanity check, repo arg ignored).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui::{AppContext as _, HeadlessAppContext, px, size};

use crate::repo::Tab;
use crate::workspace::{Popover, Workspace};

pub fn run(out: PathBuf, repo: Option<PathBuf>, view: &str) {
    let platform = gpui_platform::current_platform(true);
    let text_system = platform.text_system();
    let mut cx = HeadlessAppContext::with_platform(text_system, Arc::new(gd_ui::Assets), || {
        Some(Box::new(
            gpui_wgpu::WgpuHeadlessRenderer::new().expect("headless WGPU renderer"),
        ) as Box<dyn gpui::PlatformHeadlessRenderer>)
    });
    cx.update(|cx| {
        gd_ui::init(cx);
        crate::actions::bind(cx);
    });
    if view == "icons" {
        struct Icons;
        impl gpui::Render for Icons {
            fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
                use gpui::{ParentElement, Styled};
                gpui::div().size_full().bg(gpui::black()).flex().gap(px(10.0)).p(px(20.0))
                    .child(gd_ui::icon(gd_ui::Icon::Check).size(px(64.0)).text_color(gpui::white()))
                    .child(gd_ui::icon(gd_ui::Icon::GitBranch).size(px(64.0)).text_color(gpui::white()))
                    .child(gpui::svg().path("icons/check.svg").w(px(64.0)).h(px(64.0)).text_color(gpui::red()))
                    .child("text")
            }
        }
        let window = cx.open_window(size(px(400.0), px(120.0)), |_, cx| cx.new(|_| Icons)).unwrap();
        cx.run_until_parked();
        let counts = cx.update_window(window.into(), |_, w, _| w.rendered_primitive_counts()).ok();
        eprintln!("icon counts {counts:?}");
        cx.capture_screenshot(window.into()).unwrap().save(&out).unwrap();
        return;
    }
    if view == "watch" {
        watch_check(&mut cx, repo.expect("watch <repo>"));
        return;
    }
    let repo = if view == "welcome" { None } else { repo };
    let window = cx
        .open_window(size(px(1280.0), px(820.0)), |window, cx| {
            cx.new(|cx| Workspace::new(repo, window, cx))
        })
        .expect("open headless window");
    cx.run_until_parked();

    cx.update_window(window.into(), |_, window, cx| {
        window
            .root::<Workspace>()
            .flatten()
            .expect("workspace root")
            .update(cx, |ws, cx| {
                match view {
                    "history" => {
                        if let Some(r) = &ws.repo {
                            r.update(cx, |r, cx| r.set_tab(Tab::History, cx));
                        }
                    }
                    "repos" => ws.toggle_popover(Popover::Repositories, window, cx),
                    "branches" => ws.toggle_popover(Popover::Branches, window, cx),
                    "new-branch" => ws.toggle_popover(Popover::NewBranch, window, cx),
                    "clone" => ws.toggle_popover(Popover::Clone, window, cx),
                    _ => {}
                }
            });
    })
    .ok();
    // Let loads finish and entrance animations settle.
    for _ in 0..4 {
        cx.run_until_parked();
        cx.advance_clock(Duration::from_millis(600));
    }
    cx.run_until_parked();
    // GPUI animations/transitions run on wall-clock time (not the test
    // dispatcher clock): wait for them for real, then redraw.
    for _ in 0..3 {
        std::thread::sleep(Duration::from_millis(300));
        cx.update_window(window.into(), |_, window, _| window.refresh()).ok();
        cx.run_until_parked();
    }

    let image = cx.capture_screenshot(window.into()).expect("capture");
    image.save(&out).expect("save png");
    println!("wrote {}", out.display());
}

fn process_cpu_ticks() -> u64 {
    // utime + stime in clock ticks from /proc/self/stat (fields 14, 15).
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let rest = stat.rsplit_once(')').map(|x| x.1).unwrap_or("");
    let f: Vec<&str> = rest.split_whitespace().collect();
    f.get(11).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0)
        + f.get(12).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0)
}

/// Headless check of the file watcher. Asserts: a file edit shows up in the
/// status without any manual refresh, a burst is coalesced, and an idle repo
/// triggers zero further refreshes (no feedback loop) and ~zero CPU.
/// Real sleeps are required: the watcher thread debounces in wall-clock time,
/// and the test dispatcher only runs tasks when `run_until_parked` is called.
fn watch_check(cx: &mut HeadlessAppContext, repo: PathBuf) {
    let window = cx
        .open_window(size(px(1280.0), px(820.0)), |window, cx| {
            cx.new(|cx| Workspace::new(Some(repo.clone()), window, cx))
        })
        .expect("open headless window");
    let model = |cx: &mut HeadlessAppContext| {
        cx.update_window(window.into(), |_, w, cx| {
            w.root::<Workspace>().flatten().unwrap().read(cx).repo.clone().unwrap()
        })
        .unwrap()
    };
    let settle = |cx: &mut HeadlessAppContext, ms: u64| {
        let end = std::time::Instant::now() + Duration::from_millis(ms);
        while std::time::Instant::now() < end {
            std::thread::sleep(Duration::from_millis(50));
            cx.run_until_parked();
        }
    };
    let m = model(cx);
    settle(cx, 1000);
    let count = |cx: &mut HeadlessAppContext| cx.read_entity(&m, |r, _| r.refresh_count);
    let nfiles = |cx: &mut HeadlessAppContext| cx.read_entity(&m, |r, _| r.changes().len());

    // 1. Idle: no refreshes, no CPU.
    let (c0, t0) = (count(cx), process_cpu_ticks());
    settle(cx, 3000);
    let (c1, t1) = (count(cx), process_cpu_ticks());
    println!("idle 3s: refreshes {c0} -> {c1}, cpu ticks {} (100/s)", t1 - t0);
    assert_eq!(c0, c1, "refresh feedback loop while idle");

    // 2. A burst of edits -> exactly one debounced refresh, status updated.
    let before = nfiles(cx);
    for i in 0..20 {
        std::fs::write(repo.join(format!("watch-{i}.txt")), format!("{i}\n")).unwrap();
        std::thread::sleep(Duration::from_millis(10));
    }
    let t = std::time::Instant::now();
    while nfiles(cx) == before && t.elapsed() < Duration::from_secs(5) {
        settle(cx, 50);
    }
    println!("status updated {:?} after last write", t.elapsed());
    settle(cx, 1000);
    let (c2, after) = (count(cx), nfiles(cx));
    println!("burst of 20 files: files {before} -> {after}, refreshes {c1} -> {c2}");
    assert_eq!(after, before + 20, "watcher did not update status");
    assert_eq!(c2, c1 + 1, "burst was not coalesced into one refresh");

    // 3. Git metadata change (new branch via CLI) -> full refresh sees it.
    let ok = std::process::Command::new("git")
        .args(["branch", "watch-branch"]).current_dir(&repo).status().unwrap().success();
    assert!(ok);
    settle(cx, 1500);
    let (c3, has) = (count(cx), cx.read_entity(&m, |r, _| r.branches.iter().any(|b| b.name == "watch-branch")));
    println!("git branch: refreshes {c2} -> {c3}, branch visible: {has}");
    assert!(has, "ref change not picked up");

    // 4. Deleting files again; then idle once more.
    for i in 0..20 {
        std::fs::remove_file(repo.join(format!("watch-{i}.txt"))).unwrap();
    }
    let _ = std::process::Command::new("git").args(["branch", "-D", "watch-branch"])
        .current_dir(&repo).output();
    settle(cx, 1500);
    assert_eq!(nfiles(cx), before);
    let c4 = count(cx);
    settle(cx, 2500);
    assert_eq!(c4, count(cx), "refresh feedback loop after changes");
    println!("watch check OK (total refreshes {})", count(cx));
}
