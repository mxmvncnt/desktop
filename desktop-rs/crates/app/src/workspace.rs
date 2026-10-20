//! Root view. Layout mirrors GitHub Desktop:
//!
//! ```text
//! ┌ toolbar: [Current repository ▾] [Current branch ▾] [Fetch/Pull/Push] ┐
//! ├ sidebar (300px)           │ main                                    │
//! │  [Changes | History]      │  diff of selected file / commit        │
//! │  file list / commit list  │                                         │
//! │  commit box               │                                         │
//! └───────────────────────────┴─────────────────────────────────────────┘
//! ```
//!
//! Painted in zeron's visual language (gd-ui). Popovers are rendered as
//! `deferred` overlays with a click-catching backdrop.

use std::path::PathBuf;

use gd_git::Repository;
use gd_ui::{
    Icon, Theme, ToolbarButton, icon, motion,
    theme::{TOOLBAR_HEIGHT, scrim},
    vdivider,
};
use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, PathPromptOptions, Render, SharedString, Styled, Subscription, Task,
    UniformListScrollHandle, Window, deferred, div, px,
};
use gpui_elements::editable_text::{EditableTextState, StringStorage, TextChanged};

use crate::actions::*;
use crate::repo::{RemoteOp, RepoModel, Tab};
use crate::settings::Settings;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Popover {
    Repositories,
    Branches,
    NewBranch,
    Clone,
}

pub struct Workspace {
    pub settings: Settings,
    pub repo: Option<Entity<RepoModel>>,
    pub popover: Option<Popover>,
    /// Bumped on every popover open so its entrance animation replays.
    pub popover_gen: usize,
    pub popover_selected: usize,
    /// Error shown when no repository is open (e.g. bad path on startup).
    pub startup_error: Option<String>,

    pub summary: Entity<EditableTextState>,
    pub description: Entity<EditableTextState>,
    pub filter: Entity<EditableTextState>,
    pub name_input: Entity<EditableTextState>,
    pub url_input: Entity<EditableTextState>,

    pub changes_scroll: UniformListScrollHandle,
    pub history_scroll: UniformListScrollHandle,
    pub diff_scroll: UniformListScrollHandle,
    /// Identity of the diff shown last; scroll resets when it changes.
    pub diff_key: Option<SharedString>,
    pub commit_files_scroll: UniformListScrollHandle,
    pub popover_scroll: UniformListScrollHandle,

    focus: FocusHandle,
    notice_task: Option<Task<()>>,
    repo_sub: Option<Subscription>,
    _subs: Vec<Subscription>,
}

fn text_state(cx: &mut App) -> Entity<EditableTextState> {
    cx.new(|cx| EditableTextState::new(StringStorage::default(), cx))
}

impl Workspace {
    pub fn new(open_path: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let summary = text_state(cx);
        let description = text_state(cx);
        let filter = text_state(cx);
        let name_input = text_state(cx);
        let url_input = text_state(cx);
        let mut subs = Vec::new();
        // Re-render when inputs change (commit button enablement, filtering).
        for input in [&summary, &filter, &name_input, &url_input] {
            subs.push(cx.subscribe(input, |this: &mut Self, _, _: &TextChanged, cx| {
                this.popover_selected = 0;
                cx.notify()
            }));
        }
        // Desktop refreshes status when the window regains focus.
        subs.push(cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() {
                this.refresh(cx);
            }
        }));

        let mut this = Self {
            settings: Settings::load(),
            repo: None,
            popover: None,
            popover_gen: 0,
            popover_selected: 0,
            startup_error: None,
            summary,
            description,
            filter,
            name_input,
            url_input,
            changes_scroll: UniformListScrollHandle::new(),
            history_scroll: UniformListScrollHandle::new(),
            diff_scroll: UniformListScrollHandle::new(),
            diff_key: None,
            commit_files_scroll: UniformListScrollHandle::new(),
            popover_scroll: UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
            notice_task: None,
            repo_sub: None,
            _subs: subs,
        };
        if let Some(path) = open_path {
            this.add_repository(path, cx);
        } else if let Some(ix) = this.settings.selected {
            this.open_repository(ix, cx);
        }
        this
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if let Some(repo) = &self.repo {
            repo.update(cx, |r, cx| r.refresh(cx));
        }
    }

    // ---- repositories ------------------------------------------------------

    pub fn add_repository(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match Repository::open(&path) {
            Ok(repo) => {
                let root = repo.root().to_path_buf();
                let ix = match self.settings.repositories.iter().position(|p| *p == root) {
                    Some(ix) => ix,
                    None => {
                        self.settings.repositories.push(root);
                        self.settings.repositories.len() - 1
                    }
                };
                self.open_repository(ix, cx);
            }
            Err(e) => self.show_error(format!("{}: {e}", path.display()), cx),
        }
    }

    pub fn open_repository(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(path) = self.settings.repositories.get(ix).cloned() else {
            return;
        };
        self.settings.selected = Some(ix);
        self.settings.save();
        self.popover = None;
        match Repository::open(&path) {
            Ok(repo) => {
                let model = cx.new(|cx| RepoModel::new(repo, cx));
                self.repo_sub = Some(cx.observe(&model, |this, model, cx| {
                    // Auto-dismiss success notices after a few seconds.
                    if model.read(cx).notice.is_some() && this.notice_task.is_none() {
                        let model = model.downgrade();
                        this.notice_task = Some(cx.spawn(async move |this, cx| {
                            cx.background_executor()
                                .timer(std::time::Duration::from_secs(3))
                                .await;
                            model.update(cx, |m, cx| {
                                m.notice = None;
                                cx.notify()
                            })
                            .ok();
                            this.update(cx, |this, _| this.notice_task = None).ok();
                        }));
                    }
                    cx.notify()
                }));
                self.repo = Some(model);
            }
            Err(e) => {
                self.repo = None;
                self.show_error(format!("{}: {e}", path.display()), cx);
            }
        }
        cx.notify();
    }

    pub fn remove_repository(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix >= self.settings.repositories.len() {
            return;
        }
        self.settings.repositories.remove(ix);
        match self.settings.selected {
            Some(s) if s == ix => {
                self.settings.selected = None;
                self.repo = None;
            }
            Some(s) if s > ix => self.settings.selected = Some(s - 1),
            _ => {}
        }
        self.settings.save();
        cx.notify();
    }

    pub fn prompt_add_repository(&mut self, cx: &mut Context<Self>) {
        self.popover = None;
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Add repository".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                if let Some(path) = paths.into_iter().next() {
                    this.update(cx, |this, cx| this.add_repository(path, cx)).ok();
                }
            }
        })
        .detach();
    }

    pub fn clone_repository(&mut self, url: String, cx: &mut Context<Self>) {
        let url = url.trim().to_string();
        if url.is_empty() {
            return;
        }
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Clone into…".into()),
        });
        self.popover = None;
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else { return };
            let Some(parent) = paths.into_iter().next() else { return };
            let name = url
                .trim_end_matches('/')
                .rsplit(['/', ':'])
                .next()
                .unwrap_or("repository")
                .trim_end_matches(".git")
                .to_string();
            let dest = parent.join(name);
            this.update(cx, |this, cx| {
                this.show_notice(format!("Cloning into {}…", dest.display()), cx)
            })
            .ok();
            let res = cx
                .background_spawn({
                    let dest = dest.clone();
                    async move { Repository::clone(&url, &dest) }
                })
                .await;
            this.update(cx, |this, cx| match res {
                Ok(repo) => this.add_repository(repo.root().to_path_buf(), cx),
                Err(e) => this.show_error(format!("Clone failed: {e}"), cx),
            })
            .ok();
        })
        .detach();
    }

    fn show_error(&mut self, msg: String, cx: &mut Context<Self>) {
        if let Some(repo) = &self.repo {
            repo.update(cx, |r, cx| {
                r.error = Some(msg);
                cx.notify()
            });
        } else {
            log::error!("{msg}");
            self.startup_error = Some(msg);
            cx.notify();
        }
    }

    fn show_notice(&mut self, msg: String, cx: &mut Context<Self>) {
        if let Some(repo) = &self.repo {
            repo.update(cx, |r, cx| {
                r.notice = Some(msg);
                cx.notify()
            });
        }
    }

    // ---- popovers ----------------------------------------------------------

    pub fn toggle_popover(&mut self, p: Popover, window: &mut Window, cx: &mut Context<Self>) {
        if self.popover == Some(p) {
            self.close_popover(window, cx);
            return;
        }
        if matches!(p, Popover::Branches | Popover::NewBranch) && self.repo.is_none() {
            return;
        }
        self.popover = Some(p);
        self.popover_gen += 1;
        self.popover_selected = 0;
        for input in [&self.filter, &self.name_input, &self.url_input] {
            input.update(cx, |s, cx| s.emplace("", cx));
        }
        let target = match p {
            Popover::Repositories | Popover::Branches => &self.filter,
            Popover::NewBranch => &self.name_input,
            Popover::Clone => &self.url_input,
        };
        window.focus(&target.focus_handle(cx), cx);
        cx.notify();
    }

    pub fn close_popover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.popover = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.clone() else { return };
        let summary = self.summary.read(cx).as_str().to_string();
        let description = self.description.read(cx).as_str().to_string();
        if summary.trim().is_empty() || repo.read(cx).included_paths().is_empty() {
            return;
        }
        repo.update(cx, |r, cx| r.commit(summary, description, cx));
        self.summary.update(cx, |s, cx| s.emplace("", cx));
        self.description.update(cx, |s, cx| s.emplace("", cx));
        window.focus(&self.focus, cx);
    }

    fn remote_action(&mut self, op: RemoteOp, cx: &mut Context<Self>) {
        if let Some(repo) = &self.repo {
            repo.update(cx, |r, cx| r.remote(op, cx));
        }
    }

    fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        if let Some(repo) = &self.repo {
            repo.update(cx, |r, cx| r.set_tab(tab, cx));
        }
    }

    fn select_step(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.popover.is_some() {
            self.popover_selected = (self.popover_selected as isize + delta).max(0) as usize;
            self.popover_scroll
                .scroll_to_item(self.popover_selected, gpui::ScrollStrategy::Nearest);
            cx.notify();
            return;
        }
        let Some(repo) = self.repo.clone() else { return };
        repo.update(cx, |r, cx| {
            let step = |cur: Option<usize>, len: usize| -> Option<usize> {
                if len == 0 {
                    return None;
                }
                Some(
                    (cur.map(|c| c as isize + delta).unwrap_or(0)).clamp(0, len as isize - 1)
                        as usize,
                )
            };
            match r.tab {
                Tab::Changes => {
                    if let Some(ix) = step(r.selected_change, r.changes().len()) {
                        r.select_change(ix, cx);
                        self.changes_scroll
                            .scroll_to_item(ix, gpui::ScrollStrategy::Nearest);
                    }
                }
                Tab::History => {
                    if let Some(ix) = step(r.selected_commit, r.commits.len()) {
                        r.select_commit(ix, cx);
                        self.history_scroll
                            .scroll_to_item(ix, gpui::ScrollStrategy::Nearest);
                        if ix + 20 >= r.commits.len() {
                            r.load_more_history(cx);
                        }
                    }
                }
            }
        });
    }

    // ---- rendering ---------------------------------------------------------

    fn render_toolbar(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = Theme::of(cx).clone();
        let repo = self.repo.as_ref().map(|r| r.read(cx));
        let repo_name: SharedString = repo
            .map(|r| r.repo.name().into())
            .unwrap_or_else(|| "Select a repository".into());
        let branch: SharedString = repo
            .map(|r| r.branch_name().into())
            .unwrap_or_else(|| "—".into());

        // Desktop's contextual remote button.
        let (remote_icon, remote_caption, remote_title, remote_op, busy) = match repo {
            None => (Icon::Refresh, "No repository".into(), "Fetch origin".into(), None, false),
            Some(r) => {
                let busy = r.remote_op.is_some();
                let st = r.status.as_ref();
                let upstream = st.and_then(|s| s.upstream.clone());
                let ab = st.map(|s| s.ahead_behind).unwrap_or_default();
                let remote = upstream
                    .as_deref()
                    .and_then(|u| u.split_once('/'))
                    .map(|(r, _)| r.to_string())
                    .unwrap_or_else(|| "origin".into());
                match (r.remote_op, upstream.is_some(), ab.ahead, ab.behind) {
                    (Some(RemoteOp::Fetch), ..) => (
                        Icon::Refresh,
                        SharedString::from("Fetching…"),
                        SharedString::from(format!("Fetch {remote}")),
                        None,
                        busy,
                    ),
                    (Some(RemoteOp::Pull), ..) => (
                        Icon::ArrowDown,
                        "Pulling…".into(),
                        format!("Pull {remote}").into(),
                        None,
                        busy,
                    ),
                    (Some(RemoteOp::Push), ..) => (
                        Icon::ArrowUp,
                        "Pushing…".into(),
                        format!("Push {remote}").into(),
                        None,
                        busy,
                    ),
                    (None, false, ..) if st.and_then(|s| s.branch.as_ref()).is_some() => (
                        Icon::Cloud,
                        "Publish this branch to the remote".into(),
                        "Publish branch".into(),
                        Some(RemoteOp::Push),
                        false,
                    ),
                    (None, true, _, behind) if behind > 0 => (
                        Icon::ArrowDown,
                        format!("{behind} commit{} behind", plural(behind)).into(),
                        format!("Pull {remote}").into(),
                        Some(RemoteOp::Pull),
                        false,
                    ),
                    (None, true, ahead, _) if ahead > 0 => (
                        Icon::ArrowUp,
                        format!("{ahead} commit{} ahead", plural(ahead)).into(),
                        format!("Push {remote}").into(),
                        Some(RemoteOp::Push),
                        false,
                    ),
                    _ => (
                        Icon::Refresh,
                        "Up to date".into(),
                        format!("Fetch {remote}").into(),
                        Some(RemoteOp::Fetch),
                        false,
                    ),
                }
            }
        };

        div()
            .flex()
            .flex_none()
            .items_center()
            .h(px(TOOLBAR_HEIGHT))
            .px(px(6.0))
            .py(px(6.0))
            .gap(px(4.0))
            .bg(theme.surface)
            .border_b_1()
            .border_color(theme.border)
            .child(
                ToolbarButton::new("toolbar-repo", Icon::FolderWithFiles, "Current repository", repo_name)
                    .chevron()
                    .width(280.0)
                    .open(self.popover == Some(Popover::Repositories))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.toggle_popover(Popover::Repositories, window, cx)
                    })),
            )
            .child(div().h(px(24.0)).child(vdivider(cx)))
            .child(
                ToolbarButton::new("toolbar-branch", Icon::GitBranch, "Current branch", branch)
                    .chevron()
                    .width(260.0)
                    .open(matches!(self.popover, Some(Popover::Branches | Popover::NewBranch)))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.toggle_popover(Popover::Branches, window, cx)
                    })),
            )
            .child(div().h(px(24.0)).child(vdivider(cx)))
            .child(
                ToolbarButton::new("toolbar-remote", remote_icon, remote_caption, remote_title)
                    .width(240.0)
                    .busy(busy)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(op) = remote_op {
                            this.remote_action(op, cx)
                        }
                    })),
            )
            .child(div().flex_1())
    }

    fn render_banner(&mut self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let theme = Theme::of(cx).clone();
        let (error, notice) = match &self.repo {
            Some(r) => {
                let r = r.read(cx);
                (r.error.clone(), r.notice.clone())
            }
            None => (self.startup_error.clone(), None),
        };
        let (msg, is_error) = match (error, notice) {
            (Some(e), _) => (e, true),
            (None, Some(n)) => (n, false),
            _ => return None,
        };
        let key = format!("banner-{}", hash(&msg));
        Some(
            div()
                .absolute()
                .bottom(px(16.0))
                .right(px(16.0))
                .max_w(px(520.0))
                .child(motion::fade_in(
                    SharedString::from(key),
                    gd_ui::overlay_card(cx)
                        .flex()
                        .items_start()
                        .gap(px(10.0))
                        .p(px(12.0))
                        .child(
                            icon(if is_error { Icon::Warning } else { Icon::Check })
                                .text_color(if is_error { theme.danger } else { theme.success }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(px(12.5))
                                .text_color(theme.text)
                                .child(msg),
                        )
                        .child(gd_ui::icon_button("dismiss-banner", Icon::Close).compact().on_click(
                            cx.listener(|this, _, _, cx| {
                                this.startup_error = None;
                                if let Some(r) = &this.repo {
                                    r.update(cx, |r, cx| {
                                        r.error = None;
                                        r.notice = None;
                                        cx.notify()
                                    });
                                }
                                cx.notify()
                            }),
                        )),
                )),
        )
    }

    fn render_popover(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let p = self.popover?;
        let (left, width) = match p {
            Popover::Repositories | Popover::Clone => (8.0, 380.0),
            Popover::Branches | Popover::NewBranch => (296.0, 380.0),
        };
        let content = match p {
            Popover::Repositories => self.render_repo_list(window, cx).into_any_element(),
            Popover::Branches => self.render_branch_list(window, cx).into_any_element(),
            Popover::NewBranch => self.render_new_branch(cx).into_any_element(),
            Popover::Clone => self.render_clone(cx).into_any_element(),
        };
        Some(deferred(
            div()
                .absolute()
                .inset_0()
                .child(
                    // Backdrop: click outside closes. Very light scrim like zeron menus.
                    div()
                        .id("popover-backdrop")
                        .absolute()
                        .inset_0()
                        .bg(scrim(0.18))
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(|this, _, window, cx| this.close_popover(window, cx)),
                        ),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(TOOLBAR_HEIGHT - 2.0))
                        .left(px(left))
                        .w(px(width))
                        .child(motion::menu_in(
                            ("popover", self.popover_gen),
                            gd_ui::overlay_card(cx)
                                .id("popover-card")
                                .occlude()
                                .child(content),
                        )),
                ),
        ))
    }
}

pub fn plural(n: u32) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

impl Focusable for Workspace {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let body = match self.repo.clone() {
            Some(repo) => {
                let tab = repo.read(cx).tab;
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_sidebar(&repo, window, cx))
                    .child(match tab {
                        Tab::Changes => self.render_changes_main(&repo, cx).into_any_element(),
                        Tab::History => self.render_history_main(&repo, cx).into_any_element(),
                    })
                    .into_any_element()
            }
            None => self.render_welcome(cx).into_any_element(),
        };
        let banner = self.render_banner(cx);
        let popover = self.render_popover(window, cx);

        div()
            .id("workspace")
            .key_context(WORKSPACE)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &ShowChanges, _, cx| this.set_tab(Tab::Changes, cx)))
            .on_action(cx.listener(|this, _: &ShowHistory, _, cx| this.set_tab(Tab::History, cx)))
            .on_action(cx.listener(|this, _: &ToggleRepositoryList, w, cx| {
                this.toggle_popover(Popover::Repositories, w, cx)
            }))
            .on_action(cx.listener(|this, _: &ToggleBranchList, w, cx| {
                this.toggle_popover(Popover::Branches, w, cx)
            }))
            .on_action(cx.listener(|this, _: &NewBranch, w, cx| {
                this.toggle_popover(Popover::NewBranch, w, cx)
            }))
            .on_action(cx.listener(|this, _: &AddRepository, _, cx| this.prompt_add_repository(cx)))
            .on_action(cx.listener(|this, _: &Fetch, _, cx| this.remote_action(RemoteOp::Fetch, cx)))
            .on_action(cx.listener(|this, _: &Pull, _, cx| this.remote_action(RemoteOp::Pull, cx)))
            .on_action(cx.listener(|this, _: &Push, _, cx| this.remote_action(RemoteOp::Push, cx)))
            .on_action(cx.listener(|this, _: &Refresh, _, cx| this.refresh(cx)))
            .on_action(cx.listener(|this, _: &Commit, w, cx| this.commit(w, cx)))
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.select_step(1, cx)))
            .on_action(cx.listener(|this, _: &SelectPrevious, _, cx| this.select_step(-1, cx)))
            .on_action(cx.listener(|this, _: &Dismiss, w, cx| {
                if this.popover.is_some() {
                    this.close_popover(w, cx)
                }
            }))
            .size_full()
            .flex()
            .flex_col()
            .relative()
            .bg(theme.bg)
            .font_family(theme.font_sans.clone())
            .text_size(px(13.0))
            .text_color(theme.text)
            .child(self.render_toolbar(cx))
            .child(body)
            .children(banner)
            .children(popover)
    }
}

/// Shared styling for single-line inputs.
pub fn text_field(
    id: &'static str,
    state: &Entity<EditableTextState>,
    placeholder: &str,
    cx: &App,
) -> impl IntoElement + use<> {
    let theme = Theme::of(cx);
    gpui_elements::editable_text::text_input(id)
        .state(state.downgrade())
        .placeholder(placeholder.to_string())
        .placeholder_color(theme.text_faint)
        .selection_color(theme.selection)
        .caret_color(theme.caret)
        .caret_blink_interval_500ms()
        .w_full()
        .h(px(32.0))
        .px(px(10.0))
        .py(px(7.0))
        .rounded(px(gd_ui::theme::CONTROL_RADIUS + 1.0))
        .bg(theme.input_bg)
        .border_1()
        .border_color(theme.border)
        .text_size(px(13.0))
        .text_color(theme.text)
        .whitespace_nowrap()
        .overflow_hidden()
}

