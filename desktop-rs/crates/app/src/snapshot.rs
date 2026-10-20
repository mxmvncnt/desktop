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
//! `clone`, `welcome`, `context-menu`, `header-menu`, `discard-dialog`, `undo-strip`, `icons` (icon-rendering sanity check, repo arg ignored).

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
                    "context-menu" => ws.open_file_menu(1, gpui::point(px(120.0), px(150.0)), window, cx),
                    "header-menu" => ws.open_header_menu(gpui::point(px(60.0), px(100.0)), window, cx),
                    "discard-dialog" => {
                        if let Some(c) = ws.repo.as_ref().and_then(|r| r.read(cx).changes().get(1).cloned()) {
                            ws.dialog = Some(crate::views::menus::ConfirmDialog::DiscardFile(c));
                            cx.notify();
                        }
                    }
                    "undo-strip" => {
                        if let Some(r) = &ws.repo {
                            r.update(cx, |r, cx| {
                                r.last_commit = Some(crate::repo::LastCommit {
                                    summary: "Add module 5 with some changes".into(),
                                    description: "Body".into(),
                                    branch: r.branch_name(),
                                    time: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .map(|d| d.as_secs() as i64)
                                        .unwrap_or(0),
                                });
                                cx.notify();
                            });
                        }
                    }
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
