//! GitHub Desktop — Rust + GPUI-CE rewrite (POC). See desktop-rs/docs/.

mod actions;
mod frame_debug;
mod diff_highlight;
mod history_ops;
mod line_selection;
mod merge_ops;
mod platform;
mod repo;
mod settings;
mod stashes;
#[cfg(feature = "snapshot")]
mod snapshot;
mod views;
mod watcher;
mod workspace;

use gpui::{App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use gpui_elements::editable_text::actions::{DEFAULT_INPUT_CONTEXT, default_bindings};

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn,github_desktop=info,gd_git=info"),
    )
    .init();

    #[cfg(feature = "snapshot")]
    {
        let args: Vec<String> = std::env::args().collect();
        if args.get(1).map(String::as_str) == Some("--snapshot") {
            let out = args.get(2).map(std::path::PathBuf::from).expect("--snapshot <out.png>");
            let repo = args.get(3).map(std::path::PathBuf::from);
            let view = args.get(4).map(String::as_str).unwrap_or("changes");
            snapshot::run(out, repo, view);
            return;
        }
    }

    // `github-desktop [path]` opens/adds that repository, like `github .`.
    let open_path = std::env::args().nth(1).map(std::path::PathBuf::from);

    gpui_platform::application()
        .with_assets(gd_ui::Assets)
        .run(move |cx: &mut App| {
            gd_ui::init(cx);
            cx.bind_keys(default_bindings().as_keybindings(Some(DEFAULT_INPUT_CONTEXT)));
            actions::bind(cx);

            let bounds = Bounds::centered(None, size(px(1280.0), px(820.0)), cx);
            let mut options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("GitHub Desktop".into()),
                    ..Default::default()
                }),
                app_id: Some("github-desktop-rs".into()),
                window_min_size: Some(size(px(860.0), px(540.0))),
                ..Default::default()
            };
            platform::configure_window(&mut options);
            let window = cx
                .open_window(
                    options,
                    |window, cx| cx.new(|cx| workspace::Workspace::new(open_path, window, cx)),
                )
                .expect("failed to open window");
            window
                .update(cx, |ws, window, cx| {
                    log::info!("window decorations: {:?}", window.window_decorations());
                    ws.focus(window, cx)
                })
                .ok();
            cx.on_window_closed(|cx, _| cx.quit()).detach();
            cx.activate(true);
        });
}
