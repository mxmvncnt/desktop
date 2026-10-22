//! "Merge into current branch" dialog, the branch / conflicted-file context
//! menus, and the rename / delete / create-from dialogs. All state lives in
//! the single `Workspace.merge_ui` field; nothing here runs git on the UI thread.

use crate::shared::relative_time::relative_time;
use gd_git::{Branch, BranchKind, FileChange, MergePreview};
use gd_ui::{
    Checkbox, CheckState, ContextMenu, Dialog, Icon, MenuEntry, MenuItem, Tabs, Theme, icon, pressable,
};
use gpui::{
    AppContext, ClipboardItem, Context, Entity, Focusable, InteractiveElement, IntoElement, ParentElement,
    Pixels, Point, SharedString, Styled, Subscription, Task, UniformListScrollHandle, Window, div,
    prelude::*, px, uniform_list,
};
use gpui_elements::editable_text::{
    EditableTextState, StringStorage, TextChanged,
    actions::{Enter, Escape, NavDown, NavUp},
};

use super::in_progress_banner::file_url;
use super::pickers::sanitize_branch_name;
use crate::merge_ops::{MergeMode, Resolve};
use crate::workspace::{Workspace, plural, text_field};

const LIST_ROW: f32 = 30.0;
const LIST_ROWS: usize = 7;

pub struct MergeDialog {
    pub mode: MergeMode,
    /// Highlighted row in the filtered candidate list.
    pub ix: usize,
    pub preview: Option<MergePreview>,
    pub loading: bool,
    revision: u64,
}

pub enum BranchDialog {
    Rename { old: String },
    Delete { branch: Branch, remote: Option<String>, also_remote: bool },
    CreateFrom { base: String },
}

#[derive(Clone)]
pub enum MenuKind {
    Branch(Branch),
    Conflict(FileChange),
}

pub struct MergeMenu {
    pub position: Point<Pixels>,
    pub kind: MenuKind,
    pub selected: Option<usize>,
}

pub struct MergeUi {
    pub dialog: Option<MergeDialog>,
    pub branch_dialog: Option<BranchDialog>,
    pub menu: Option<MergeMenu>,
    menu_gen: usize,
    pub filter: Entity<EditableTextState>,
    pub input: Entity<EditableTextState>,
    scroll: UniformListScrollHandle,
    preview_task: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl MergeUi {
    pub fn new(cx: &mut Context<Workspace>) -> Self {
        let mk = |cx: &mut Context<Workspace>| {
            cx.new(|cx| EditableTextState::new(StringStorage::default(), cx))
        };
        let (filter, input) = (mk(cx), mk(cx));
        let subs = vec![
            cx.subscribe(&filter, |this: &mut Workspace, _, _: &TextChanged, cx| {
                if this.merge_ui.dialog.is_some() {
                    this.merge_select(0, cx);
                }
            }),
            cx.subscribe(&input, |_, _, _: &TextChanged, cx| cx.notify()),
        ];
        Self {
            dialog: None,
            branch_dialog: None,
            menu: None,
            menu_gen: 0,
            filter,
            input,
            scroll: UniformListScrollHandle::new(),
            preview_task: None,
            _subs: subs,
        }
    }

    /// Whether the root should carry the "Menu" key context (↑/↓/Enter/Esc
    /// reach the workspace instead of an input).
    pub fn wants_menu_context(&self) -> bool {
        self.menu.is_some() || matches!(self.branch_dialog, Some(BranchDialog::Delete { .. }))
    }
}

// ---- menu rows ------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Act {
    Merge,
    Rebase,
    CreateFrom,
    Rename,
    CopyName,
    Delete,
    Ours,
    Theirs,
    Resolved,
    OpenEditor,
    CopyPath,
    Reveal,
}

struct Row {
    act: Act,
    label: String,
    icon: Icon,
    danger: bool,
    disabled: bool,
}

fn row(act: Act, label: impl Into<String>, icon: Icon, disabled: bool) -> Option<Row> {
    Some(Row { act, label: label.into(), icon, danger: false, disabled })
}

fn menu_rows(kind: &MenuKind, paused: bool) -> Vec<Option<Row>> {
    match kind {
        MenuKind::Branch(b) => {
            let remote = b.kind == BranchKind::Remote;
            vec![
                row(Act::Merge, "Merge into current branch…", Icon::Fork, b.is_head || paused),
                row(Act::Rebase, "Rebase onto this branch…", Icon::Restart, b.is_head || paused),
                None,
                row(Act::CreateFrom, "Create branch from this…", Icon::Plus, false),
                row(Act::Rename, "Rename…", Icon::Pen, remote),
                row(Act::CopyName, "Copy branch name", Icon::Copy, false),
                None,
                Some(Row {
                    act: Act::Delete,
                    label: if remote { "Delete remote branch…" } else { "Delete…" }.into(),
                    icon: Icon::Trash,
                    danger: true,
                    // git refuses to delete the checked-out branch.
                    disabled: b.is_head,
                }),
            ]
        }
        MenuKind::Conflict(_) => vec![
            row(Act::Ours, "Use ours", Icon::ArrowLeft, false),
            row(Act::Theirs, "Use theirs", Icon::ArrowRight, false),
            row(Act::Resolved, "Mark as resolved", Icon::Check, false),
            None,
            row(Act::OpenEditor, "Open in default editor", Icon::Document, false),
            None,
            row(Act::CopyPath, "Copy file path", Icon::Copy, false),
            row(Act::Reveal, "Reveal in file manager", Icon::Folder, false),
        ],
    }
}

fn selectable(r: &Option<Row>) -> bool {
    matches!(r, Some(r) if !r.disabled)
}

impl Workspace {
    fn paused(&self, cx: &gpui::App) -> bool {
        self.repo.as_ref().is_some_and(|r| r.read(cx).merge.in_progress.is_some())
    }

    // ---- merge dialog -----------------------------------------------------

    /// Branches that can be merged: everything but the current branch and
    /// `origin/HEAD`; remote branches with a local twin are hidden.
    fn merge_candidates(&self, cx: &gpui::App) -> Vec<Branch> {
        let Some(repo) = &self.repo else { return Vec::new() };
        let f = self.merge_ui.filter.read(cx).as_str().trim().to_lowercase();
        let r = repo.read(cx);
        let local: std::collections::HashSet<&str> = r
            .branches
            .iter()
            .filter(|b| b.kind == BranchKind::Local)
            .map(|b| b.name.as_str())
            .collect();
        r.branches
            .iter()
            .filter(|b| !b.is_head)
            .filter(|b| f.is_empty() || b.name.to_lowercase().contains(&f))
            .filter(|b| {
                b.kind == BranchKind::Local
                    || !b.name.split_once('/').is_some_and(|(_, n)| local.contains(n))
            })
            .cloned()
            .collect()
    }

    pub fn open_merge_dialog(
        &mut self,
        mode: MergeMode,
        preselect: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.repo.is_none() || self.paused(cx) {
            return;
        }
        self.popover = None;
        self.menu = None;
        self.dialog = None;
        self.merge_ui.menu = None;
        self.merge_ui.branch_dialog = None;
        self.merge_ui.filter.update(cx, |s, cx| s.emplace("", cx));
        self.merge_ui.dialog = Some(MergeDialog { mode, ix: 0, preview: None, loading: false, revision: 0 });
        let ix = preselect
            .and_then(|name| self.merge_candidates(cx).iter().position(|b| b.name == name))
            .unwrap_or(0);
        self.merge_select(ix, cx);
        self.merge_ui.scroll.scroll_to_item(ix, gpui::ScrollStrategy::Nearest);
        window.focus(&self.merge_ui.filter.focus_handle(cx), cx);
        cx.notify();
    }

    pub fn merge_select(&mut self, ix: usize, cx: &mut Context<Self>) {
        let cands = self.merge_candidates(cx);
        let ix = ix.min(cands.len().saturating_sub(1));
        let branch = cands.get(ix).map(|b| b.name.clone());
        let Some(d) = &mut self.merge_ui.dialog else { return };
        d.ix = ix;
        d.revision += 1;
        d.preview = None;
        d.loading = branch.is_some();
        let generation = d.revision;
        self.merge_ui.preview_task = None;
        cx.notify();
        let (Some(branch), Some(repo)) = (branch, self.repo.clone()) else { return };
        let git = repo.read(cx).repo.clone();
        self.merge_ui.preview_task = Some(cx.spawn(async move |this, cx| {
            let res = cx.background_spawn(async move { git.merge_preview(&branch) }).await;
            this.update(cx, |this, cx| {
                if let Some(d) = &mut this.merge_ui.dialog {
                    if d.revision == generation {
                        d.loading = false;
                        match res {
                            Ok(p) => d.preview = Some(p),
                            Err(e) => {
                                if let Some(r) = &this.repo {
                                    r.update(cx, |r, cx| {
                                        r.error = Some(e.to_string());
                                        cx.notify()
                                    });
                                }
                            }
                        }
                        cx.notify();
                    }
                }
            })
            .ok();
        }));
    }

    fn merge_step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let n = self.merge_candidates(cx).len();
        let Some(d) = &self.merge_ui.dialog else { return };
        if n == 0 {
            return;
        }
        let ix = (d.ix as isize + delta).clamp(0, n as isize - 1) as usize;
        self.merge_ui.scroll.scroll_to_item(ix, gpui::ScrollStrategy::Nearest);
        self.merge_select(ix, cx);
    }

    fn merge_set_mode(&mut self, mode: MergeMode, cx: &mut Context<Self>) {
        if let Some(d) = &mut self.merge_ui.dialog {
            d.mode = mode;
            cx.notify();
        }
    }

    /// Whether the dialog's primary action can run (branch has commits to bring in).
    fn merge_ready(&self) -> bool {
        self.merge_ui
            .dialog
            .as_ref()
            .and_then(|d| d.preview.as_ref())
            .is_some_and(|p| p.ahead > 0)
    }

    pub fn merge_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.merge_ready() {
            return;
        }
        let Some(d) = self.merge_ui.dialog.take() else { return };
        let (Some(p), Some(repo)) = (d.preview, self.repo.clone()) else { return };
        self.focus(window, cx);
        repo.update(cx, |r, cx| r.start_merge(d.mode, p.branch, cx));
        cx.notify();
    }

    /// Close any merge-UI overlay. Returns whether one was open.
    pub fn close_merge_ui(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let open = self.merge_ui.dialog.is_some()
            || self.merge_ui.branch_dialog.is_some()
            || self.merge_ui.menu.is_some();
        // Close one layer at a time: dialog / menu top-most first.
        if self.merge_ui.menu.take().is_none() {
            self.merge_ui.dialog = None;
            self.merge_ui.branch_dialog = None;
            self.merge_ui.preview_task = None;
            self.focus(window, cx);
        }
        if open {
            cx.notify();
        }
        open
    }

    // ---- context menus ----------------------------------------------------

    pub fn open_branch_menu(
        &mut self,
        branch: Branch,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_merge_menu(MenuKind::Branch(branch), position, window, cx);
    }

    pub fn open_conflict_menu(
        &mut self,
        change: FileChange,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.menu = None;
        self.open_merge_menu(MenuKind::Conflict(change), position, window, cx);
    }

    fn open_merge_menu(
        &mut self,
        kind: MenuKind,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.merge_ui.menu = Some(MergeMenu { position, kind, selected: None });
        self.merge_ui.menu_gen += 1;
        self.focus(window, cx);
        cx.notify();
    }

    pub fn merge_menu_step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let paused = self.paused(cx);
        let Some(menu) = &mut self.merge_ui.menu else { return };
        let rows = menu_rows(&menu.kind, paused);
        let n = rows.len() as isize;
        let mut ix = menu.selected.map(|s| s as isize).unwrap_or(if delta > 0 { -1 } else { n });
        for _ in 0..n {
            ix = (ix + delta).rem_euclid(n);
            if selectable(&rows[ix as usize]) {
                menu.selected = Some(ix as usize);
                break;
            }
        }
        cx.notify();
    }

    /// Enter while a merge-UI menu or confirm dialog is open. Returns whether handled.
    pub fn merge_ui_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if let Some(menu) = &self.merge_ui.menu {
            let paused = self.paused(cx);
            let act = menu
                .selected
                .and_then(|s| menu_rows(&menu.kind, paused).into_iter().nth(s))
                .flatten()
                .filter(|r| !r.disabled)
                .map(|r| r.act);
            if let Some(a) = act {
                self.run_merge_menu(a, window, cx);
            }
            return true;
        }
        if matches!(self.merge_ui.branch_dialog, Some(BranchDialog::Delete { .. })) {
            self.branch_dialog_confirm(window, cx);
            return true;
        }
        false
    }

    /// Snapshot helper: run the n-th kind of branch-menu action (1 rename, 2 delete).
    #[cfg(feature = "snapshot")]
    pub fn snapshot_menu_action(&mut self, which: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.run_merge_menu(if which == 1 { Act::Rename } else { Act::Delete }, window, cx);
    }

    fn run_merge_menu(&mut self, act: Act, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.merge_ui.menu.take() else { return };
        cx.notify();
        let Some(repo) = self.repo.clone() else { return };
        let root = repo.read(cx).repo.root().to_path_buf();
        match (act, menu.kind) {
            (Act::Merge, MenuKind::Branch(b)) => {
                self.open_merge_dialog(MergeMode::Merge, Some(b.name), window, cx)
            }
            (Act::Rebase, MenuKind::Branch(b)) => {
                self.open_merge_dialog(MergeMode::Rebase, Some(b.name), window, cx)
            }
            (Act::CopyName, MenuKind::Branch(b)) => {
                cx.write_to_clipboard(ClipboardItem::new_string(b.name))
            }
            (Act::CreateFrom, MenuKind::Branch(b)) => {
                self.open_branch_dialog(BranchDialog::CreateFrom { base: b.name }, "", window, cx)
            }
            (Act::Rename, MenuKind::Branch(b)) => {
                let old = b.name.clone();
                self.open_branch_dialog(BranchDialog::Rename { old: old.clone() }, &old, window, cx)
            }
            (Act::Delete, MenuKind::Branch(b)) => {
                // Offer deleting the upstream too when it still exists.
                let remote = (b.kind == BranchKind::Local)
                    .then(|| b.upstream.clone())
                    .flatten()
                    .filter(|u| {
                        repo.read(cx)
                            .branches
                            .iter()
                            .any(|x| x.kind == BranchKind::Remote && &x.name == u)
                    });
                self.open_branch_dialog(BranchDialog::Delete { branch: b, remote, also_remote: false }, "", window, cx)
            }
            (Act::Ours, MenuKind::Conflict(c)) => {
                repo.update(cx, |r, cx| r.resolve_file(c.path, Resolve::Ours, cx))
            }
            (Act::Theirs, MenuKind::Conflict(c)) => {
                repo.update(cx, |r, cx| r.resolve_file(c.path, Resolve::Theirs, cx))
            }
            (Act::Resolved, MenuKind::Conflict(c)) => {
                repo.update(cx, |r, cx| r.resolve_file(c.path, Resolve::Marked, cx))
            }
            (Act::OpenEditor, MenuKind::Conflict(c)) => cx.open_url(&file_url(&root.join(&c.path))),
            (Act::CopyPath, MenuKind::Conflict(c)) => {
                cx.write_to_clipboard(ClipboardItem::new_string(root.join(&c.path).display().to_string()))
            }
            (Act::Reveal, MenuKind::Conflict(c)) => {
                let abs = root.join(&c.path);
                let target = if abs.exists() { abs } else { root.clone() };
                cx.reveal_path(&target);
            }
            _ => {}
        }
    }

    // ---- branch dialogs -----------------------------------------------------

    fn open_branch_dialog(
        &mut self,
        dialog: BranchDialog,
        prefill: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delete = matches!(dialog, BranchDialog::Delete { .. });
        self.popover = None;
        self.merge_ui.input.update(cx, |s, cx| s.emplace(prefill, cx));
        self.merge_ui.branch_dialog = Some(dialog);
        if delete {
            self.focus(window, cx);
        } else {
            window.focus(&self.merge_ui.input.focus_handle(cx), cx);
        }
        cx.notify();
    }

    /// Name typed into the rename / create-from input, sanitized.
    fn typed_branch_name(&self, cx: &gpui::App) -> String {
        sanitize_branch_name(self.merge_ui.input.read(cx).as_str())
    }

    fn branch_dialog_valid(&self, cx: &gpui::App) -> bool {
        let name = self.typed_branch_name(cx);
        let exists = |n: &str| {
            self.repo
                .as_ref()
                .is_some_and(|r| r.read(cx).branches.iter().any(|b| b.kind == BranchKind::Local && b.name == n))
        };
        match &self.merge_ui.branch_dialog {
            Some(BranchDialog::Rename { old }) => !name.is_empty() && &name != old && !exists(&name),
            Some(BranchDialog::CreateFrom { .. }) => !name.is_empty() && !exists(&name),
            Some(BranchDialog::Delete { .. }) => true,
            None => false,
        }
    }

    pub fn branch_dialog_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.branch_dialog_valid(cx) {
            return;
        }
        let name = self.typed_branch_name(cx);
        let Some(dialog) = self.merge_ui.branch_dialog.take() else { return };
        self.focus(window, cx);
        cx.notify();
        let Some(repo) = self.repo.clone() else { return };
        repo.update(cx, |r, cx| match dialog {
            BranchDialog::Rename { old } => r.rename_branch(old, name, cx),
            BranchDialog::CreateFrom { base } => r.create_branch_from(name, base, cx),
            BranchDialog::Delete { branch, remote, also_remote } => {
                let remote_target = if branch.kind == BranchKind::Remote {
                    branch.name.split_once('/').map(|(rm, b)| (rm.to_string(), b.to_string()))
                } else if also_remote {
                    remote.and_then(|u| u.split_once('/').map(|(rm, b)| (rm.to_string(), b.to_string())))
                } else {
                    None
                };
                r.delete_branch(branch, remote_target, cx)
            }
        });
    }

    // ---- rendering ------------------------------------------------------------

    /// All merge-UI overlays (each only exists while open).
    pub fn render_merge_ui(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let menu = self.render_merge_menu(cx);
        let dialog = self.render_merge_dialog(cx);
        let branch_dialog = self.render_branch_dialog(cx);
        div().absolute().inset_0().children(menu).children(dialog).children(branch_dialog)
    }

    fn render_merge_menu(&mut self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let paused = self.paused(cx);
        let menu = self.merge_ui.menu.as_ref()?;
        let ws = cx.entity();
        let mut m = ContextMenu::new(("merge-menu", self.merge_ui.menu_gen), menu.position)
            .selected(menu.selected)
            .on_dismiss({
                let ws = ws.clone();
                move |_, cx| {
                    ws.update(cx, |this, cx| {
                        this.merge_ui.menu = None;
                        cx.notify()
                    })
                }
            });
        for r in menu_rows(&menu.kind, paused) {
            m = m.entries([match r {
                None => MenuEntry::Separator,
                Some(r) => {
                    let ws = ws.clone();
                    let act = r.act;
                    let mut item = MenuItem::new(r.label)
                        .icon(r.icon)
                        .disabled(r.disabled)
                        .on_select(move |w, cx| ws.update(cx, |this, cx| this.run_merge_menu(act, w, cx)));
                    if r.danger {
                        item = item.danger();
                    }
                    MenuEntry::Item(item)
                }
            }]);
        }
        Some(m)
    }

    fn render_merge_dialog(&mut self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let theme = Theme::of(cx).clone();
        let d = self.merge_ui.dialog.as_ref()?;
        let (mode, ix, loading) = (d.mode, d.ix, d.loading);
        let preview = d.preview.clone();
        let current = self.repo.as_ref()?.read(cx).branch_name();
        let cands = self.merge_candidates(cx);
        let n = cands.len();
        let ready = self.merge_ready();
        let ws = cx.entity();

        let title = match mode {
            MergeMode::Merge => format!("Merge into {current}"),
            MergeMode::Squash => format!("Squash and merge into {current}"),
            MergeMode::Rebase => format!("Rebase {current}"),
        };
        let confirm = match mode {
            MergeMode::Merge => "Create a merge commit",
            MergeMode::Squash => "Squash and merge",
            MergeMode::Rebase => "Rebase",
        };

        let tabs = {
            let ws = ws.clone();
            Tabs::new("merge-mode", mode as usize)
                .tab("Merge", None)
                .tab("Squash and merge", None)
                .tab("Rebase", None)
                .on_select(move |i, _, cx| {
                    let mode = [MergeMode::Merge, MergeMode::Squash, MergeMode::Rebase][i];
                    ws.update(cx, |this, cx| this.merge_set_mode(mode, cx))
                })
        };

        let list = if n == 0 {
            div()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.0))
                .text_color(theme.text_faint)
                .child("No matching branches")
                .into_any_element()
        } else {
            let rows = cands.clone();
            let ws = ws.clone();
            uniform_list("merge-branches", n, move |range, _w, cx| {
                let theme = Theme::of(cx).clone();
                range
                    .filter_map(|i| {
                        let b = rows.get(i)?;
                        let ws = ws.clone();
                        Some(
                            pressable(("merge-branch", i), i == ix, cx)
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                .h(px(LIST_ROW))
                                .w_full()
                                .px(px(8.0))
                                .rounded(px(6.0))
                                .child(
                                    icon(if b.kind == BranchKind::Remote { Icon::Cloud } else { Icon::GitBranch })
                                        .size(px(14.0))
                                        .text_color(theme.text_faint),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_color(if i == ix { theme.text } else { theme.text_muted })
                                        .child(b.name.clone()),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .text_size(px(11.0))
                                        .text_color(theme.text_faint)
                                        .child(relative_time(b.tip_time)),
                                )
                                .on_click(move |_, _, cx| ws.update(cx, |this, cx| this.merge_select(i, cx))),
                        )
                    })
                    .collect()
            })
            .track_scroll(&self.merge_ui.scroll)
            .size_full()
            .into_any_element()
        };

        let status = preview_line(mode, &current, preview.as_ref(), loading, n > 0, &theme);
        let content = div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .mt(px(6.0))
            .child(tabs)
            .child(text_field("merge-filter", &self.merge_ui.filter, "Filter branches", cx))
            .child(
                div()
                    .h(px(LIST_ROW * LIST_ROWS as f32 + 8.0))
                    .py(px(4.0))
                    .px(px(2.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(theme.border)
                    .bg(gd_ui::theme::ink(0.02))
                    .child(list),
            )
            .child(div().min_h(px(36.0)).child(status));

        let (ws_ok, ws_cancel) = (ws.clone(), ws.clone());
        let dialog = Dialog::new("merge-dialog", title)
            .width(520.0)
            .content(content)
            .confirm_label(confirm)
            .confirm_disabled(!ready)
            .on_confirm(move |w, cx| ws_ok.update(cx, |this, cx| this.merge_confirm(w, cx)))
            .on_cancel(move |w, cx| {
                ws_cancel.update(cx, |this, cx| {
                    this.close_merge_ui(w, cx);
                })
            });
        Some(
            div()
                .absolute()
                .inset_0()
                .capture_action(cx.listener(|this, _: &Enter, w, cx| {
                    cx.stop_propagation();
                    this.merge_confirm(w, cx)
                }))
                .capture_action(cx.listener(|this, _: &NavDown, _, cx| {
                    cx.stop_propagation();
                    this.merge_step(1, cx)
                }))
                .capture_action(cx.listener(|this, _: &NavUp, _, cx| {
                    cx.stop_propagation();
                    this.merge_step(-1, cx)
                }))
                .capture_action(cx.listener(|this, _: &Escape, w, cx| {
                    cx.stop_propagation();
                    this.close_merge_ui(w, cx);
                }))
                .child(dialog),
        )
    }

    fn render_branch_dialog(&mut self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let theme = Theme::of(cx).clone();
        let valid = self.branch_dialog_valid(cx);
        let typed = self.typed_branch_name(cx);
        let current = self.repo.as_ref()?.read(cx).branch_name();
        let d = self.merge_ui.branch_dialog.as_ref()?;
        let ws = cx.entity();
        let (ws_ok, ws_cancel) = (ws.clone(), ws.clone());

        let dialog = match d {
            BranchDialog::Rename { old } => Dialog::new("rename-branch", format!("Rename {old}"))
                .body("Choose a new name for this branch.")
                .content(div().mt(px(4.0)).child(text_field("rename-input", &self.merge_ui.input, "Branch name", cx)))
                .detail(if typed.is_empty() || typed == *old {
                    SharedString::from("Spaces become dashes.")
                } else {
                    format!("The branch will be renamed to {typed}.").into()
                })
                .confirm_label("Rename branch"),
            BranchDialog::CreateFrom { base } => Dialog::new("create-from", format!("Create branch from {base}"))
                .body(format!("The new branch starts at {base} and is checked out."))
                .content(div().mt(px(4.0)).child(text_field("create-from-input", &self.merge_ui.input, "Branch name", cx)))
                .detail(format!("Currently on {current}. Uncommitted changes come along."))
                .confirm_label("Create branch"),
            BranchDialog::Delete { branch, remote, also_remote } => {
                let is_remote = branch.kind == BranchKind::Remote;
                let mut dlg = Dialog::new(
                    "delete-branch",
                    if is_remote { "Delete remote branch" } else { "Delete branch" },
                )
                .body(if is_remote {
                    format!("Delete {} from the remote? Other people lose it too.", branch.name)
                } else {
                    format!("Are you sure you want to delete the branch {}?", branch.name)
                })
                .detail("This action cannot be undone.")
                .danger()
                .confirm_label(if is_remote { "Delete remote branch" } else { "Delete branch" });
                if let (Some(remote), false) = (remote, is_remote) {
                    let ws = ws.clone();
                    let state = if *also_remote { CheckState::Checked } else { CheckState::Unchecked };
                    dlg = dlg.content(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .mt(px(4.0))
                            .text_size(px(13.0))
                            .text_color(theme.text)
                            .child(Checkbox::new("delete-remote", state).on_click(move |_, _, cx| {
                                ws.update(cx, |this, cx| {
                                    if let Some(BranchDialog::Delete { also_remote, .. }) =
                                        &mut this.merge_ui.branch_dialog
                                    {
                                        *also_remote = !*also_remote;
                                        cx.notify();
                                    }
                                })
                            }))
                            .child(format!("Also delete the remote branch {remote}")),
                    );
                }
                dlg
            }
        };
        let dialog = dialog
            .confirm_disabled(!valid)
            .on_confirm(move |w, cx| ws_ok.update(cx, |this, cx| this.branch_dialog_confirm(w, cx)))
            .on_cancel(move |w, cx| {
                ws_cancel.update(cx, |this, cx| {
                    this.close_merge_ui(w, cx);
                })
            });
        Some(
            div()
                .absolute()
                .inset_0()
                .capture_action(cx.listener(|this, _: &Enter, w, cx| {
                    cx.stop_propagation();
                    this.branch_dialog_confirm(w, cx)
                }))
                .capture_action(cx.listener(|this, _: &Escape, w, cx| {
                    cx.stop_propagation();
                    this.close_merge_ui(w, cx);
                }))
                .child(dialog),
        )
    }
}

/// Desktop-style preview sentence under the list.
fn preview_line(
    mode: MergeMode,
    current: &str,
    p: Option<&MergePreview>,
    loading: bool,
    has_candidates: bool,
    theme: &Theme,
) -> gpui::Div {
    let mut line = div().flex().flex_wrap().items_center().gap_x(px(4.0)).text_size(px(12.5));
    let say = |parts: Vec<(String, bool)>, line: gpui::Div| {
        parts.into_iter().fold(line, |l, (t, bold)| {
            l.child(div().text_color(if bold { theme.text } else { theme.text_muted }).when(bold, |d| {
                d.font_weight(gpui::FontWeight::MEDIUM)
            }).child(t))
        })
    };
    let s = |t: &str| (t.to_string(), false);
    let b = |t: String| (t, true);
    let Some(p) = p else {
        let text = if loading {
            "Checking the branch…"
        } else if has_candidates {
            ""
        } else {
            "There are no other branches to merge."
        };
        return line.text_color(theme.text_faint).child(text.to_string());
    };
    let commits = |n: u32| format!("{n} commit{}", plural(n));
    let parts = match (mode, p.ahead) {
        (MergeMode::Rebase, 0) => vec![b(current.to_string()), s("is already based on"), b(format!("{}.", p.branch))],
        (_, 0) => vec![s("This branch is up to date with"), b(format!("{current}."))],
        (MergeMode::Merge, n) => vec![
            s("This will merge"),
            b(commits(n)),
            s("from"),
            b(p.branch.clone()),
            s("into"),
            b(format!("{current}.")),
        ],
        (MergeMode::Squash, n) => vec![
            s("This will squash"),
            b(commits(n)),
            s("from"),
            b(p.branch.clone()),
            s("into one commit on"),
            b(format!("{current}.")),
        ],
        (MergeMode::Rebase, _) if p.behind == 0 => {
            vec![s("This will fast-forward"), b(current.to_string()), s("to"), b(format!("{}.", p.branch))]
        }
        (MergeMode::Rebase, _) => vec![
            s("This will replay"),
            b(commits(p.behind)),
            s("of"),
            b(current.to_string()),
            s("on top of"),
            b(format!("{}.", p.branch)),
        ],
    };
    line = say(parts, line);
    let mut out = div().flex().flex_col().gap(px(3.0)).child(line);
    if p.ahead > 0 {
        match &p.conflicts {
            Some(c) if !c.is_empty() => {
                out = out.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .text_size(px(12.0))
                        .text_color(theme.warning)
                        .child(icon(Icon::Warning).size(px(13.0)).text_color(theme.warning))
                        .child(format!(
                            "{} conflicted file{} expected. You'll resolve them next.",
                            c.len(),
                            plural(c.len() as u32)
                        )),
                );
            }
            Some(_) if p.behind > 0 && mode != MergeMode::Rebase => {
                out = out.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .text_size(px(12.0))
                        .text_color(theme.success)
                        .child(icon(Icon::Check).size(px(13.0)).text_color(theme.success))
                        .child("No conflicts expected."),
                );
            }
            _ => {}
        }
    }
    out
}
