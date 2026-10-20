//! `gd-git` — the git layer for the Rust rewrite of GitHub Desktop.
//!
//! Design (see `docs/ARCHITECTURE.md` §Git layer):
//! - Shells out to the user's `git` binary, the same strategy as the Electron
//!   app's `dugite`. Hooks, credential helpers, config and LFS behave exactly
//!   like the CLI, and we avoid libgit2's partial feature coverage.
//! - Every function is **blocking**. The UI runs them on GPUI's background
//!   executor (`cx.background_spawn`) and never on the main thread.
//! - Machine-readable formats only (`status --porcelain=v2 -z`, NUL-separated
//!   `log` formats) so parsing is cheap and locale-independent.

mod diff;
mod exec;
mod log;
mod refs;
mod status;

pub use diff::{Diff, DiffHunk, DiffLine, DiffLineKind};
pub use exec::GitError;
pub use log::{Commit, CommitFile};
pub use refs::{Branch, BranchKind};
pub use status::{AheadBehind, FileChange, FileStatus, WorkingStatus};

use std::path::{Path, PathBuf};

use exec::Git;

pub type Result<T> = std::result::Result<T, GitError>;

/// A handle to a repository on disk. Cheap to clone; holds no open resources.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Repository {
    root: PathBuf,
}

impl Repository {
    /// Open the repository containing `path` (any subdirectory works).
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let out = Git::new(path.as_ref())
            .args(["rev-parse", "--show-toplevel"])
            .run()?;
        let root = PathBuf::from(out.trim_end_matches(['\n', '\r']));
        Ok(Self { root })
    }

    /// Initialize a new repository at `path` (creating the directory).
    pub fn init(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        std::fs::create_dir_all(path).map_err(|e| GitError::Io(e.to_string()))?;
        Git::new(path).args(["init", "-q"]).run()?;
        Self::open(path)
    }

    /// Clone `url` into `dest`. Blocks for the whole transfer.
    pub fn clone(url: &str, dest: impl AsRef<Path>) -> Result<Self> {
        let dest = dest.as_ref();
        let parent = dest.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent).map_err(|e| GitError::Io(e.to_string()))?;
        Git::new(parent)
            .args(["clone", "--progress", "--", url])
            .arg(dest.as_os_str())
            .network()
            .run()?;
        Self::open(dest)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Directory name, used as the display name in the repository list.
    pub fn name(&self) -> String {
        self.root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.root.display().to_string())
    }

    pub(crate) fn git(&self) -> Git {
        Git::new(&self.root)
    }

    /// Whether `HEAD` resolves to a commit (false in a freshly-initialized repo).
    pub fn has_head(&self) -> bool {
        self.git()
            .args(["rev-parse", "--verify", "-q", "HEAD"])
            .run()
            .is_ok()
    }

    // ---- write operations -------------------------------------------------

    /// Commit exactly `paths` with the given message, mirroring Desktop's
    /// "checkbox" model: the index is reset to HEAD and only the selected
    /// paths are staged, so whatever the user ticked is what gets committed.
    pub fn commit(&self, summary: &str, description: &str, paths: &[String]) -> Result<String> {
        if summary.trim().is_empty() {
            return Err(GitError::Invalid("commit summary is empty".into()));
        }
        if paths.is_empty() {
            return Err(GitError::Invalid("no files selected".into()));
        }
        if self.has_head() {
            self.git().args(["reset", "-q", "--", "."]).run()?;
        } else {
            // No HEAD yet: unstage everything by removing it from the index only.
            let _ = self
                .git()
                .args(["rm", "-q", "-r", "--cached", "--ignore-unmatch", "--", "."])
                .run();
        }
        for chunk in paths.chunks(500) {
            self.git()
                .args(["add", "-A", "--"])
                .args(chunk.iter().map(String::as_str))
                .run()?;
        }
        let mut message = summary.trim().to_string();
        if !description.trim().is_empty() {
            message.push_str("\n\n");
            message.push_str(description.trim());
        }
        self.git()
            .args(["commit", "-q", "-F", "-"])
            .stdin(message)
            .run()?;
        Ok(self
            .git()
            .args(["rev-parse", "HEAD"])
            .run()?
            .trim()
            .to_string())
    }

    /// Discard working-tree changes for `paths`. Untracked files are deleted.
    pub fn discard(&self, changes: &[FileChange]) -> Result<()> {
        let (untracked, tracked): (Vec<_>, Vec<_>) = changes
            .iter()
            .partition(|c| c.status == FileStatus::Untracked);
        if !tracked.is_empty() {
            let paths: Vec<&str> = tracked.iter().map(|c| c.path.as_str()).collect();
            if self.has_head() {
                self.git()
                    .args(["checkout", "-q", "HEAD", "--"])
                    .args(paths.iter().copied())
                    .run()?;
            } else {
                self.git()
                    .args(["rm", "-q", "--cached", "--"])
                    .args(paths.iter().copied())
                    .run()?;
            }
        }
        for c in untracked {
            let p = self.root.join(&c.path);
            let res = if p.is_dir() {
                std::fs::remove_dir_all(&p)
            } else {
                std::fs::remove_file(&p)
            };
            res.map_err(|e| GitError::Io(format!("{}: {e}", c.path)))?;
        }
        Ok(())
    }

    /// Switch to an existing local branch, or create a tracking branch for a
    /// remote one (`origin/foo` → `foo`).
    pub fn checkout(&self, branch: &Branch) -> Result<()> {
        match branch.kind {
            BranchKind::Local => self.git().args(["checkout", "-q", &branch.name]).run()?,
            BranchKind::Remote => {
                let local = branch
                    .name
                    .split_once('/')
                    .map(|(_, rest)| rest)
                    .unwrap_or(&branch.name);
                self.git()
                    .args(["checkout", "-q", "-b", local, "--track", &branch.name])
                    .run()?
            }
        };
        Ok(())
    }

    /// Create a branch from HEAD and switch to it.
    pub fn create_branch(&self, name: &str) -> Result<()> {
        self.git().args(["checkout", "-q", "-b", name]).run()?;
        Ok(())
    }

    pub fn delete_branch(&self, name: &str, force: bool) -> Result<()> {
        let flag = if force { "-D" } else { "-d" };
        self.git().args(["branch", flag, name]).run()?;
        Ok(())
    }

    pub fn fetch(&self) -> Result<()> {
        self.git()
            .args(["fetch", "--prune", "--all"])
            .network()
            .run()?;
        Ok(())
    }

    pub fn pull(&self) -> Result<()> {
        self.git().args(["pull", "--ff"]).network().run()?;
        Ok(())
    }

    /// Push the current branch, setting upstream on first push like Desktop's
    /// "Publish branch".
    pub fn push(&self, branch: &str, has_upstream: bool) -> Result<()> {
        if has_upstream {
            self.git().args(["push"]).network().run()?;
        } else {
            let remote = self.default_remote()?.unwrap_or_else(|| "origin".into());
            self.git()
                .args(["push", "--set-upstream", &remote, branch])
                .network()
                .run()?;
        }
        Ok(())
    }

    /// Undo the last commit, keeping its changes in the working tree.
    pub fn undo_last_commit(&self) -> Result<()> {
        self.git().args(["reset", "-q", "--soft", "HEAD~1"]).run()?;
        Ok(())
    }

    pub fn remotes(&self) -> Result<Vec<(String, String)>> {
        let out = self.git().args(["remote", "-v"]).run()?;
        let mut v: Vec<(String, String)> = out
            .lines()
            .filter(|l| l.ends_with("(fetch)"))
            .filter_map(|l| {
                let mut it = l.split_whitespace();
                Some((it.next()?.to_string(), it.next()?.to_string()))
            })
            .collect();
        v.dedup();
        Ok(v)
    }

    fn default_remote(&self) -> Result<Option<String>> {
        let remotes = self.remotes()?;
        Ok(remotes
            .iter()
            .find(|(n, _)| n == "origin")
            .or(remotes.first())
            .map(|(n, _)| n.clone()))
    }
}
