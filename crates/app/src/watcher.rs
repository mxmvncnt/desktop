//! File-system watcher for the open repository (see docs/ARCHITECTURE.md
//! §File watcher).
//!
//! One OS thread per open repository owns the `notify` watcher and a trailing
//! debounce. It blocks on a channel while idle (zero CPU) and sends a single
//! "something changed" tick to the UI side once a burst of events has been
//! quiet for [`DEBOUNCE`]. `bool` payload: `true` when git metadata (HEAD,
//! index, refs, ...) changed too, so branches/history need reloading;
//! `false` when only working-tree files changed.
//!
//! Dropping [`RepoWatcher`] stops the thread and drops the OS watches.
//!
//! Layout of the watches (instead of one recursive watch on the root, which
//! would also cover `.git/objects` with its 256 fan-out dirs and `target/`):
//! - the root, non-recursive (to notice new top-level entries),
//! - every top-level directory except `.git`, `target`, `node_modules`, recursively,
//! - the git dir non-recursively plus `refs/` recursively.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gd_git::Repository;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// Quiet period after the last event before a refresh is requested.
pub const DEBOUNCE: Duration = Duration::from_millis(300);
/// A continuous stream of events still triggers a refresh at least this often.
const MAX_WAIT: Duration = Duration::from_secs(2);

/// Top-level directories never watched (noise; `git status` stays the truth).
const SKIPPED_DIRS: [&str; 3] = [".git", "target", "node_modules"];
/// Files directly inside the git dir whose changes matter.
const GIT_FILES: [&str; 11] = [
    "HEAD",
    "index",
    "FETCH_HEAD",
    "MERGE_HEAD",
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
    "REBASE_HEAD",
    "ORIG_HEAD",
    "packed-refs",
    "MERGE_MSG",
    "config",
];

enum Msg {
    Event(notify::Result<Event>),
    Stop,
}

pub struct RepoWatcher {
    tx: mpsc::Sender<Msg>,
}

impl Drop for RepoWatcher {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Stop);
    }
}

impl RepoWatcher {
    /// Start watching. Ticks arrive on the returned receiver; it closes when
    /// the watcher thread exits.
    pub fn start(repo: &Repository) -> (Self, smol::channel::Receiver<bool>) {
        let (tx, rx) = mpsc::channel::<Msg>();
        let (out_tx, out_rx) = smol::channel::unbounded::<bool>();
        let paths = Paths {
            root: repo.root().to_path_buf(),
            git_dir: repo.git_dir(),
            common_dir: repo.common_git_dir(),
        };
        let cb_tx = tx.clone();
        std::thread::Builder::new()
            .name("fs-watcher".into())
            .spawn(move || run(paths, cb_tx, rx, out_tx))
            .expect("spawn fs-watcher thread");
        (Self { tx }, out_rx)
    }
}

#[derive(Clone)]
struct Paths {
    root: PathBuf,
    git_dir: PathBuf,
    common_dir: PathBuf,
}

fn run(paths: Paths, cb_tx: mpsc::Sender<Msg>, rx: mpsc::Receiver<Msg>, out: smol::channel::Sender<bool>) {
    let mut watcher: RecommendedWatcher = match notify::recommended_watcher(move |ev| {
        let _ = cb_tx.send(Msg::Event(ev));
    }) {
        Ok(w) => w,
        Err(e) => {
            log::warn!("fs watcher unavailable: {e}");
            return;
        }
    };
    add_initial_watches(&mut watcher, &paths);

    // (time of first pending event, time of last, git metadata changed)
    let mut pending: Option<(Instant, Instant, bool)> = None;
    loop {
        let msg = match pending {
            None => match rx.recv() {
                Ok(m) => m,
                Err(_) => return,
            },
            Some((first, last, full)) => {
                let deadline = (last + DEBOUNCE).min(first + MAX_WAIT);
                match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(m) => m,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        pending = None;
                        log::debug!("fs watcher tick (full={full})");
                        if out.send_blocking(full).is_err() {
                            return;
                        }
                        continue;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
        };
        let ev = match msg {
            Msg::Stop => return,
            Msg::Event(Ok(ev)) => ev,
            Msg::Event(Err(e)) => {
                log::debug!("fs watcher error: {e}");
                continue;
            }
        };
        log::trace!("fs event: {:?}", ev.paths);
        // Reads (`git status` opening files) must never count as changes,
        // otherwise our own refresh would retrigger itself.
        if !matches!(
            ev.kind,
            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) | EventKind::Any
        ) {
            continue;
        }
        let mut hit: Option<bool> = None;
        for p in &ev.paths {
            if let Some(full) = classify(&paths, p) {
                hit = Some(hit.unwrap_or(false) || full);
                // A new top-level directory needs its own recursive watch.
                if matches!(ev.kind, EventKind::Create(_)) && p.parent() == Some(paths.root.as_path()) {
                    watch_top_level(&mut watcher, p);
                }
            }
        }
        if let Some(full) = hit {
            let now = Instant::now();
            pending = Some(match pending {
                Some((first, _, f)) => (first, now, f || full),
                None => (now, now, full),
            });
        }
    }
}

fn add_initial_watches(w: &mut RecommendedWatcher, p: &Paths) {
    let _ = w.watch(&p.root, RecursiveMode::NonRecursive);
    if let Ok(rd) = std::fs::read_dir(&p.root) {
        for entry in rd.flatten() {
            watch_top_level(w, &entry.path());
        }
    }
    for dir in [&p.git_dir, &p.common_dir] {
        let _ = w.watch(dir, RecursiveMode::NonRecursive);
        let refs = dir.join("refs");
        if refs.is_dir() {
            let _ = w.watch(&refs, RecursiveMode::Recursive);
        }
        if p.git_dir == p.common_dir {
            break;
        }
    }
}

fn watch_top_level(w: &mut RecommendedWatcher, path: &Path) {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    // `is_dir` follows symlinks; don't recurse through links (cycles).
    let is_real_dir = std::fs::symlink_metadata(path).map(|m| m.is_dir()).unwrap_or(false);
    if is_real_dir && !SKIPPED_DIRS.contains(&name) {
        let _ = w.watch(path, RecursiveMode::Recursive);
    }
}

/// `Some(full)` if an event on `path` should trigger a refresh.
fn classify(p: &Paths, path: &Path) -> Option<bool> {
    for dir in [&p.git_dir, &p.common_dir] {
        if let Ok(rel) = path.strip_prefix(dir) {
            return classify_git(rel).then_some(true);
        }
    }
    let rel = path.strip_prefix(&p.root).ok()?;
    let mut comps = rel.components().map(|c| c.as_os_str().to_str().unwrap_or(""));
    let first = comps.next()?;
    if first == ".git" || first == "target" {
        return None;
    }
    if rel.components().any(|c| c.as_os_str() == "node_modules") {
        return None;
    }
    Some(false)
}

/// `rel` is relative to the git dir.
fn classify_git(rel: &Path) -> bool {
    let mut comps = rel.components().map(|c| c.as_os_str().to_str().unwrap_or(""));
    match comps.next() {
        Some("refs") => !rel.to_str().is_some_and(|s| s.ends_with(".lock")),
        Some(name) if comps.next().is_none() => {
            GIT_FILES.contains(&name) || name.starts_with("rebase-")
        }
        // objects/, logs/, hooks/, lfs/, modules/ ... and any nested path.
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> Paths {
        Paths {
            root: "/r".into(),
            git_dir: "/r/.git".into(),
            common_dir: "/r/.git".into(),
        }
    }

    #[test]
    fn classifies() {
        let p = paths();
        assert_eq!(classify(&p, Path::new("/r/src/a.rs")), Some(false));
        assert_eq!(classify(&p, Path::new("/r/target/debug/x")), None);
        assert_eq!(classify(&p, Path::new("/r/web/node_modules/x/y")), None);
        assert_eq!(classify(&p, Path::new("/r/.git/index")), Some(true));
        assert_eq!(classify(&p, Path::new("/r/.git/index.lock")), None);
        assert_eq!(classify(&p, Path::new("/r/.git/HEAD")), Some(true));
        assert_eq!(classify(&p, Path::new("/r/.git/refs/heads/main")), Some(true));
        assert_eq!(classify(&p, Path::new("/r/.git/refs/heads/main.lock")), None);
        assert_eq!(classify(&p, Path::new("/r/.git/objects/ab/cdef")), None);
        assert_eq!(classify(&p, Path::new("/r/.git/logs/HEAD")), None);
        assert_eq!(classify(&p, Path::new("/r/.git/FETCH_HEAD")), Some(true));
        assert_eq!(classify(&p, Path::new("/r/.git/rebase-merge")), Some(true));
    }
}
