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
//! Views: `changes` (default), `changes@N` (N-th file selected), `history`, `repos`, `branches`, `new-branch`,
//! `clone`, `welcome`, `watch` (headless file-watcher check, no PNG), `context-menu`,
//! `header-menu`, `discard-dialog`, `split`, `split-history`, `split-ws`, `hide-ws`, `diff-options-menu`, `undo-strip`,
//! `stash-list`, `stash-detail`, `stash-detail-untracked`, `stash-all`, `stash-menu`, `stash-dialog`, `stash-drop-dialog`,
//! `switch-branch-dialog`,
//! `merge-dialog`, `squash-dialog`, `rebase-dialog`, `branch-menu`, `branch-menu-current`, `delete-dialog`,
//! `rename-dialog`, `expanded`/`expanded-split` (hunk expansion), `partial[@N]`/`partial-split[@N]` (line selection: file N, some rows deselected; hover with `GD_SNAPSHOT_MOUSE=x,y`), `history-menu`/`-menu-head`/`-menu-pushed`/`-multiselect`/`-multimenu`/`-squash`/`-reset`/`-cherry`/`-tag`/`-checkout`, `amend-mode` (need a repo with a remote, see PROGRESS.md), `conflict-menu` (use a conflicted repo made with `merge_smoke -- <dir> --keep-conflict`;
//! plain `changes` then shows the banner), `icons` (icon-rendering sanity check, repo arg ignored).

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
                    v if v.starts_with("changes@") => {
                        // `changes@N`: select the N-th changed file.
                        let n = v[8..].parse().unwrap_or(0);
                        if let Some(r) = &ws.repo {
                            r.update(cx, |r, cx| r.select_change(n, cx));
                        }
                    }
                    v if v.starts_with("history") || v == "amend-mode" => {
                        if let Some(r) = &ws.repo {
                            r.update(cx, |r, cx| r.set_tab(Tab::History, cx));
                        }
                    }
                    // Diff options (set directly: no settings file is written).
                    "split" | "split-history" | "hide-ws" | "split-ws" | "diff-options-menu" => {
                        ws.settings.diff_split = matches!(view, "split" | "split-history" | "split-ws" | "diff-options-menu");
                        ws.settings.diff_ignore_whitespace = matches!(view, "hide-ws" | "split-ws");
                        ws.apply_diff_options(cx);
                        if view == "split-history" {
                            if let Some(r) = &ws.repo {
                                r.update(cx, |r, cx| r.set_tab(Tab::History, cx));
                            }
                        }
                        if view == "diff-options-menu" {
                            ws.open_diff_options(gpui::point(px(1240.0), px(60.0)), window, cx);
                        }
                        cx.notify();
                    }
                    // Long-line handling: wrapped, or scrolled 160px right (both columns in split).
                    "wrap" | "split-wrap" | "hscroll" | "split-hscroll" => {
                        ws.settings.diff_split = view.starts_with("split");
                        ws.settings.diff_wrap = view.ends_with("wrap");
                        if view.ends_with("hscroll") {
                            ws.diff_view.hscroll = px(160.0);
                        }
                        cx.notify();
                    }
                    // Theme/settings page, set directly (no settings file is written).
                    "settings" | "settings-light" | "settings-diff" | "changes-light" => {
                        ws.settings.theme = if view.ends_with("light") {
                            crate::settings::ThemeMode::Light
                        } else {
                            crate::settings::ThemeMode::Dark
                        };
                        ws.apply_theme(window, cx);
                        ws.settings_open = view.starts_with("settings");
                        if view == "settings-diff" {
                            ws.settings_page = crate::views::settings::SettingsPage::Diff;
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
                    "stash-list" => {}
                    "stash-detail" => {
                        if let Some(r) = &ws.repo {
                            r.update(cx, |r, cx| {
                                if let Some(s) = r.stashes.list.first().cloned() {
                                    r.stashes.all_branches = true;
                                    r.select_stash(s.sha, cx);
                                }
                            });
                        }
                    }
                    "stash-detail-untracked" => {
                        if let Some(r) = &ws.repo {
                            r.update(cx, |r, cx| {
                                if let Some(s) = r.stashes.list.last().cloned() {
                                    r.stashes.all_branches = true;
                                    r.select_stash(s.sha, cx);
                                }
                            });
                        }
                    }
                    "stash-all" => {
                        if let Some(r) = &ws.repo {
                            r.update(cx, |r, cx| { r.stashes.all_branches = true; cx.notify() });
                        }
                    }
                    "stash-menu" => {
                        if let Some(s) = ws.repo.as_ref().and_then(|r| r.read(cx).stashes.list.first().cloned()) {
                            ws.open_stash_menu(s, gpui::point(px(120.0), px(560.0)), window, cx);
                        }
                    }
                    "stash-dialog" => {
                        if let Some(r) = &ws.repo {
                            r.update(cx, |r, _| r.excluded.insert("untracked.md".into()));
                        }
                        ws.open_stash_create(false, window, cx);
                    }
                    "stash-drop-dialog" => {
                        if let Some(s) = ws.repo.as_ref().and_then(|r| r.read(cx).stashes.list.first().cloned()) {
                            ws.stash_ui.dialog = Some(crate::views::stashes::StashDialog::Drop(s));
                            cx.notify();
                        }
                    }
                    "switch-branch-dialog" => {
                        if let Some(b) = ws.repo.as_ref().and_then(|r| r.read(cx).branches.iter().find(|b| !b.is_head).cloned()) {
                            ws.open_switch_dialog(b, cx);
                        }
                    }
                    // ---- merge / conflicts (see views/merge.rs) ----
                    "merge-dialog" | "squash-dialog" | "rebase-dialog" => {
                        let mode = match view {
                            "squash-dialog" => crate::merge_ops::MergeMode::Squash,
                            "rebase-dialog" => crate::merge_ops::MergeMode::Rebase,
                            _ => crate::merge_ops::MergeMode::Merge,
                        };
                        ws.open_merge_dialog(mode, None, window, cx);
                    }
                    "branch-menu" | "branch-menu-current" | "delete-dialog" | "rename-dialog" => {
                        ws.toggle_popover(Popover::Branches, window, cx);
                        let want_head = view == "branch-menu-current";
                        let b = ws.repo.as_ref().and_then(|r| {
                            r.read(cx).branches.iter().find(|b| b.is_head == want_head && b.kind == gd_git::BranchKind::Local).cloned()
                        });
                        if let Some(b) = b {
                            ws.open_branch_menu(b, gpui::point(px(420.0), px(150.0)), window, cx);
                            match view {
                                "delete-dialog" => ws.snapshot_menu_action(2, window, cx),
                                "rename-dialog" => ws.snapshot_menu_action(1, window, cx),
                                _ => {}
                            }
                        }
                    }
                    "conflict-menu" => ws.open_file_menu(0, gpui::point(px(120.0), px(150.0)), window, cx),
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
    // `partial[-split][@N]`: file N with some lines deselected and hunk 0's handle hovered.
    if view.starts_with("partial") {
        let n = view.split_once('@').and_then(|(_, n)| n.parse().ok()).unwrap_or(0);
        cx.update_window(window.into(), |_, window, cx| {
            let ws = window.root::<Workspace>().flatten().expect("workspace root");
            ws.update(cx, |ws, cx| {
                ws.settings.diff_split = view.starts_with("partial-split");
                ws.apply_diff_options(cx);
                if let Some(r) = &ws.repo {
                    r.update(cx, |r, cx| r.select_change(n, cx));
                }
            });
        })
        .ok();
        cx.run_until_parked();
        cx.update_window(window.into(), |_, window, cx| {
            let ws = window.root::<Workspace>().flatten().expect("workspace root");
            ws.update(cx, |ws, cx| {
                if let Some(r) = &ws.repo {
                    r.update(cx, |r, cx| {
                        let Some(diff) = r.change_diff.clone() else { return };
                        let rows: Vec<usize> = (0..diff.lines.len()).filter(|&i| gd_git::selectable(&diff, i)).collect();
                        // Deselect the 2nd selectable row and the last hunk.
                        if let Some(&ix) = rows.get(1) {
                            r.toggle_row(ix, cx);
                        }
                        // ...and the last change block.
                        if let Some(b) = rows.last().and_then(|&ix| crate::repo::RepoModel::change_block(&diff, ix)) {
                            r.toggle_block(b, cx);
                        }
                    });
                }
                cx.notify();
            });
        })
        .ok();
    }
    // `expanded[-split]`: file 0 with hunk 0 expanded down once and the gap
    // above the last hunk expanded fully (see `gd_git::expand`).
    if view.starts_with("expanded") {
        cx.update_window(window.into(), |_, window, cx| {
            let ws = window.root::<Workspace>().flatten().expect("workspace root");
            ws.update(cx, |ws, cx| {
                ws.settings.diff_split = view == "expanded-split";
                ws.apply_diff_options(cx);
            });
        })
        .ok();
        cx.run_until_parked();
        cx.update_window(window.into(), |_, window, cx| {
            let ws = window.root::<Workspace>().flatten().expect("workspace root");
            ws.update(cx, |ws, cx| {
                if let Some(r) = &ws.repo {
                    r.update(cx, |r, cx| {
                        let last = r.change_diff.as_ref().map_or(0, |d| d.hunks.len().saturating_sub(1));
                        r.expand_diff(false, gd_git::Expansion::Down(0), cx);
                        r.expand_diff(false, gd_git::Expansion::All(last), cx);
                    });
                }
            });
        })
        .ok();
    }
    // Phase 2: views that need the history list loaded first.
    if view.starts_with("history-") || view == "amend-mode" {
        cx.run_until_parked();
        cx.update_window(window.into(), |_, window, cx| {
            let ws = window.root::<Workspace>().flatten().expect("workspace root");
            ws.update(cx, |ws, cx| history_phase2(ws, view, window, cx));
        })
        .ok();
        cx.run_until_parked();
    }
    // `GD_SNAPSHOT_MOUSE=x,y` (window px): park the pointer there to capture hover states.
    if let Some((x, y)) = std::env::var("GD_SNAPSHOT_MOUSE")
        .ok()
        .and_then(|v| v.split_once(',').and_then(|(x, y)| Some((x.parse::<f32>().ok()?, y.parse::<f32>().ok()?))))
    {
        cx.run_until_parked();
        for _ in 0..2 {
            cx.update_window(window.into(), |_, window, cx| {
                window.dispatch_event(
                    gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
                        position: gpui::point(px(x), px(y)),
                        pressed_button: None,
                        modifiers: Default::default(),
                    }),
                    cx,
                );
            })
            .ok();
            cx.run_until_parked();
        }
    }
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

/// History views: `history-menu`, `history-menu-head`, `history-multiselect`,
/// `history-squash`, `history-reset`, `history-cherry`, `history-tag`,
/// `history-checkout`, `amend-mode`.
fn history_phase2(ws: &mut Workspace, view: &str, window: &mut gpui::Window, cx: &mut gpui::Context<Workspace>) {
    use crate::views::history_menu::HAction;
    let Some(repo) = ws.repo.clone() else { return };
    let at = gpui::point(px(150.0), px(190.0));
    let multi = |cx: &mut gpui::Context<Workspace>| {
        repo.update(cx, |r, cx| {
            r.click_commit(1, false, false, cx);
            r.click_commit(3, false, true, cx);
        })
    };
    match view {
        "history-menu" => ws.open_commit_menu(2, at, window, cx),
        "history-menu-head" => ws.open_commit_menu(0, at, window, cx),
        "history-menu-pushed" => ws.open_commit_menu(6, at, window, cx),
        "history-multiselect" => multi(cx),
        "history-multimenu" => {
            multi(cx);
            ws.open_commit_menu(2, at, window, cx);
        }
        "history-squash" => {
            multi(cx);
            ws.open_commit_menu(2, at, window, cx);
            ws.run_hist_action(HAction::Squash, window, cx);
        }
        "history-reset" => {
            ws.open_commit_menu(2, at, window, cx);
            ws.run_hist_action(HAction::Reset, window, cx);
        }
        "history-cherry" => {
            ws.open_commit_menu(2, at, window, cx);
            ws.run_hist_action(HAction::CherryPick, window, cx);
        }
        "history-tag" => {
            ws.open_commit_menu(2, at, window, cx);
            ws.run_hist_action(HAction::CreateTag, window, cx);
            ws.history_ui.name.update(cx, |s, cx| s.emplace("v1.2.0", cx));
        }
        "history-checkout" => {
            ws.open_commit_menu(2, at, window, cx);
            ws.run_hist_action(HAction::Checkout, window, cx);
        }
        "amend-mode" => {
            let head = repo.read(cx).commits.first().cloned();
            if let Some(head) = head {
                ws.start_amend(head, window, cx);
            }
        }
        _ => {}
    }
    cx.notify();
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
