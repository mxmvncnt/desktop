//! Pull-request preview operations on `RepoModel`. All git work runs on the
//! background executor; the generation counter drops stale loads.

use std::sync::Arc;

use gd_git::{BranchKind, Comparison, Diff, FileChange};
use gpui::{AppContext, Context};

use crate::features::diff::highlight_diff;
use crate::repo::RepoModel;

/// What one background load returns.
struct Loaded {
    base: Option<String>,
    comparison: Option<Comparison>,
    remote_url: Option<String>,
    first: Option<(Diff, String)>,
}

impl RepoModel {
    pub fn open_pr_preview(&mut self, cx: &mut Context<Self>) {
        self.pr.open = true;
        self.pr.base_menu = false;
        self.load_pr_comparison(cx);
    }

    pub fn close_pr_preview(&mut self, cx: &mut Context<Self>) {
        self.pr.open = false;
        self.pr.base_menu = false;
        self.pr.task = None;
        cx.notify();
    }

    pub fn toggle_pr_base_menu(&mut self, cx: &mut Context<Self>) {
        self.pr.base_menu = !self.pr.base_menu;
        cx.notify();
    }

    pub fn set_pr_base(&mut self, base: String, cx: &mut Context<Self>) {
        self.pr.base = Some(base);
        self.pr.base_menu = false;
        self.load_pr_comparison(cx);
    }

    /// Branch name on the remote for the PR head: the upstream's name when
    /// the branch is published, else the local name.
    pub fn pr_head_name(&self) -> String {
        let upstream = self.status.as_ref().and_then(|s| s.upstream.clone());
        match upstream.and_then(|u| u.split_once('/').map(|(_, b)| b.to_string())) {
            Some(b) => b,
            None => self.branch_name(),
        }
    }

    /// `base` as the remote knows it (`origin/main` -> `main`).
    pub fn pr_base_on_remote(&self, base: &str) -> String {
        match self.branches.iter().find(|b| b.name == base) {
            Some(b) if b.kind == BranchKind::Remote => base.split_once('/').map_or(base, |(_, n)| n).to_string(),
            _ => base.to_string(),
        }
    }

    /// (Re)load the comparison of HEAD against the chosen or default base.
    pub fn load_pr_comparison(&mut self, cx: &mut Context<Self>) {
        self.pr.gen_ += 1;
        let generation = self.pr.gen_;
        self.pr.loading = true;
        self.pr.error = None;
        let (repo, chosen, opts) = (self.repo.clone(), self.pr.base.clone(), self.diff_options);
        self.pr.task = Some(cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move {
                    let base = chosen.or_else(|| repo.default_base_branch());
                    let remote_url = repo.tracking_remote_url();
                    let Some(b) = base.clone() else {
                        return Ok(Loaded { base, comparison: None, remote_url, first: None });
                    };
                    let cmp = repo.compare(&b, "HEAD")?;
                    let first = match cmp.files.first() {
                        Some(f) => Some((repo.compare_file_diff(&cmp.merge_base, "HEAD", f, opts)?, f.path.clone())),
                        None => None,
                    };
                    Ok::<_, gd_git::GitError>(Loaded { base, comparison: Some(cmp), remote_url, first })
                })
                .await;
            this.update(cx, |this, cx| {
                if this.pr.gen_ != generation {
                    return;
                }
                this.pr.loading = false;
                match res {
                    Ok(l) => this.apply_pr_loaded(l, cx),
                    Err(e) => {
                        this.pr.comparison = None;
                        this.pr.error = Some(e.to_string());
                        this.pr.diff = None;
                    }
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn apply_pr_loaded(&mut self, l: Loaded, cx: &mut Context<Self>) {
        self.pr.loaded_base = l.base.clone();
        self.pr.remote_url = l.remote_url;
        if self.pr.base.is_none() {
            self.pr.base = l.base;
        }
        let files = l.comparison.as_ref().map(|c| c.files.len()).unwrap_or(0);
        self.pr.selected_file = (files > 0).then_some(0);
        self.pr.comparison = l.comparison;
        self.set_pr_diff(l.first, cx);
    }

    pub fn select_pr_file(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(cmp) = &self.pr.comparison else { return };
        let Some(file) = cmp.files.get(ix).cloned() else { return };
        let merge_base = cmp.merge_base.clone();
        self.pr.selected_file = Some(ix);
        self.pr.gen_ += 1;
        let generation = self.pr.gen_;
        let (repo, opts) = (self.repo.clone(), self.diff_options);
        self.pr.task = Some(cx.spawn(async move |this, cx| {
            let res = cx
                .background_spawn(async move { pr_file_diff(&repo, &merge_base, &file, opts) })
                .await;
            this.update(cx, |this, cx| {
                if this.pr.gen_ != generation {
                    return;
                }
                match res {
                    Ok(d) => this.set_pr_diff(Some(d), cx),
                    Err(e) => this.error = Some(e.to_string()),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn set_pr_diff(&mut self, diff: Option<(Diff, String)>, cx: &mut Context<Self>) {
        self.pr.hl = None;
        self.pr.hl_task = None;
        let Some((d, path)) = diff else {
            self.pr.diff = None;
            return;
        };
        let d = Arc::new(d);
        self.pr.diff = Some(d.clone());
        let cache = self.pr.cache.clone();
        self.pr.hl_task = Some(cx.spawn(async move |this, cx| {
            let for_task = d.clone();
            let hl = cx.background_spawn(async move { highlight_diff(&for_task, &path, &cache) }).await;
            this.update(cx, |this, cx| {
                if hl.is_some() && this.pr.diff.as_ref().is_some_and(|c| Arc::ptr_eq(c, &d)) {
                    this.pr.hl = hl;
                    cx.notify();
                }
            })
            .ok();
        }));
    }
}

fn pr_file_diff(
    repo: &gd_git::Repository,
    merge_base: &str,
    file: &FileChange,
    opts: gd_git::DiffOptions,
) -> gd_git::Result<(Diff, String)> {
    repo.compare_file_diff(merge_base, "HEAD", file, opts).map(|d| (d, file.path.clone()))
}
