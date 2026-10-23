//! `Workspace::new`: inputs, window subscriptions, first repository.

use std::path::PathBuf;
use std::time::Duration;

use gpui::{App, AppContext, Context, Entity, Focusable, Subscription, UniformListScrollHandle, Window};
use gpui_elements::editable_text::{EditableTextState, StringStorage, TextChanged};

use super::state::Workspace;
use crate::features::branches::BranchUi;
use crate::features::history::HistoryUi;
use crate::features::merge::MergeUi;
use crate::features::settings::{Settings, ThemeMode};
use crate::features::stashes::StashUi;

/// Skip the focus refresh when the watcher refreshed this recently.
const RECENT_WATCH_REFRESH: Duration = Duration::from_secs(1);

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
        let mut subs = input_subscriptions([&summary, &filter, &name_input, &url_input], cx);
        subs.extend(window_subscriptions(window, cx));

        let mut this = Self {
            settings: Settings::load(),
            settings_open: false,
            settings_page: Default::default(),
            repo: None,
            popover: None,
            popover_gen: 0,
            popover_dismissed: None,
            popover_selected: 0,
            menu: None,
            menu_gen: 0,
            dialog: None,
            history_ui: HistoryUi::new(cx),
            stash_ui: StashUi::new(cx),
            merge_ui: MergeUi::new(cx),
            branch_ui: BranchUi::new(cx),
            startup_error: None,
            corner_radius: 0.0,
            summary,
            description,
            filter,
            name_input,
            new_branch_base: None,
            new_branch_base_open: false,
            url_input,
            changes_scroll: UniformListScrollHandle::new(),
            history_scroll: UniformListScrollHandle::new(),
            diff_scroll: UniformListScrollHandle::new(),
            diff_key: None,
            diff_view: Default::default(),
            diff_select: Default::default(),
            commit_files_scroll: UniformListScrollHandle::new(),
            popover_scroll: UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
            notice_task: None,
            repo_sub: None,
            repo_event_sub: None,
            _subs: subs,
        };
        this.apply_theme(window, cx);
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
}

impl Focusable for Workspace {
    fn focus_handle(&self, _: &App) -> gpui::FocusHandle {
        self.focus.clone()
    }
}

/// Re-render when inputs change (commit button enablement, filtering).
fn input_subscriptions(
    inputs: [&Entity<EditableTextState>; 4],
    cx: &mut Context<Workspace>,
) -> Vec<Subscription> {
    inputs
        .into_iter()
        .map(|input| {
            cx.subscribe(input, |this: &mut Workspace, _, _: &TextChanged, cx| {
                this.reset_popover_selection(cx);
                cx.notify()
            })
        })
        .collect()
}

fn window_subscriptions(window: &mut Window, cx: &mut Context<Workspace>) -> [Subscription; 2] {
    [
        // Desktop refreshes status when the window regains focus.
        cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() {
                // The watcher already keeps us fresh; skip if it just refreshed.
                let recent = this.repo.as_ref().is_some_and(|r| {
                    r.read(cx).last_watch_refresh.is_some_and(|t| t.elapsed() < RECENT_WATCH_REFRESH)
                });
                if !recent {
                    this.refresh(cx);
                }
            }
        }),
        cx.observe_window_appearance(window, |this, window, cx| {
            if this.settings.theme == ThemeMode::System {
                this.apply_theme(window, cx);
            }
        }),
    ]
}
