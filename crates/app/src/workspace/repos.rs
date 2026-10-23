//! `impl Workspace`: adding, opening, removing and cloning repositories.

use std::path::PathBuf;

use gd_git::Repository;
use gpui::{AppContext, Context, PathPromptOptions};

use super::state::Workspace;
use crate::features::notices::observe_notices;
use crate::repo::{RepoEvent, RepoModel};

impl Workspace {
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
            Ok(repo) => self.attach_model(repo, cx),
            Err(e) => {
                self.repo = None;
                self.show_error(format!("{}: {e}", path.display()), cx);
            }
        }
        cx.notify();
    }

    /// Build the `RepoModel` for `repo`, wire its observers, make it current.
    fn attach_model(&mut self, repo: Repository, cx: &mut Context<Self>) {
        let model = cx.new(|cx| RepoModel::new(repo, cx));
        model.update(cx, |m, _| {
            m.diff_options.ignore_whitespace = self.settings.diff_ignore_whitespace
        });
        self.repo_sub = Some(cx.observe(&model, |this, model, cx| observe_notices(this, model, cx)));
        self.repo_event_sub = Some(cx.subscribe(&model, |this, _, ev: &RepoEvent, cx| this.on_repo_event(ev, cx)));
        self.repo = Some(model);
        self.menu = None;
        self.dialog = None;
    }

    fn on_repo_event(&mut self, ev: &RepoEvent, cx: &mut Context<Self>) {
        match ev {
            RepoEvent::CommitUndone { summary, description } => self.fill_commit_inputs(summary, description, cx),
            RepoEvent::SwitchBranchWithChanges(branch) => self.open_switch_dialog(branch.clone(), cx),
            RepoEvent::PreparedMessage { summary, description } => {
                // A merge paused: show git's prepared message unless the user typed one.
                if self.summary.read(cx).as_str().trim().is_empty() {
                    self.fill_commit_inputs(summary, description, cx);
                }
            }
        }
    }

    fn fill_commit_inputs(&mut self, summary: &str, description: &str, cx: &mut Context<Self>) {
        self.summary.update(cx, |s, cx| s.emplace(summary, cx));
        self.description.update(cx, |s, cx| s.emplace(description, cx));
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
        let paths = cx.prompt_for_paths(directory_prompt("Add repository"));
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
        let paths = cx.prompt_for_paths(directory_prompt("Clone into…"));
        self.popover = None;
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else { return };
            let Some(parent) = paths.into_iter().next() else { return };
            let dest = parent.join(clone_dir_name(&url));
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
}

fn directory_prompt(prompt: &'static str) -> PathPromptOptions {
    PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some(prompt.into()) }
}

/// Folder name `git clone` would pick: last path segment without `.git`.
fn clone_dir_name(url: &str) -> String {
    url.trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .unwrap_or("repository")
        .trim_end_matches(".git")
        .to_string()
}
