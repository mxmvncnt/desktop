//! Working-directory status via `git status --porcelain=v2 -z --branch`.

use crate::{GitError, Repository, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileStatus {
    New,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Conflicted,
    Untracked,
    TypeChanged,
}

impl FileStatus {
    /// Single-letter badge used in the file list.
    pub fn badge(self) -> &'static str {
        match self {
            Self::New | Self::Untracked => "+",
            Self::Modified | Self::TypeChanged => "•",
            Self::Deleted => "−",
            Self::Renamed | Self::Copied => "→",
            Self::Conflicted => "!",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileChange {
    /// Path relative to the repository root.
    pub path: String,
    /// Previous path for renames/copies.
    pub old_path: Option<String>,
    pub status: FileStatus,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AheadBehind {
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkingStatus {
    /// Current branch name; `None` when HEAD is detached.
    pub branch: Option<String>,
    /// Commit HEAD points to; `None` before the first commit.
    pub head_oid: Option<String>,
    pub upstream: Option<String>,
    /// Only meaningful when `upstream` is set.
    pub ahead_behind: AheadBehind,
    pub files: Vec<FileChange>,
}

impl Repository {
    pub fn status(&self) -> Result<WorkingStatus> {
        let out = self
            .git()
            .args([
                "status",
                "--porcelain=v2",
                "-z",
                "--branch",
                "--untracked-files=all",
                "--ignore-submodules=dirty",
            ])
            .run_bytes()?;
        parse_status(&out)
    }
}

fn status_from_xy(x: u8, y: u8) -> FileStatus {
    // Desktop shows the combined (index + worktree) state relative to HEAD.
    match (x, y) {
        (b'A', _) => FileStatus::New,
        (b'D', _) | (_, b'D') => FileStatus::Deleted,
        (b'R', _) => FileStatus::Renamed,
        (b'C', _) => FileStatus::Copied,
        (b'T', _) | (_, b'T') => FileStatus::TypeChanged,
        _ => FileStatus::Modified,
    }
}

pub(crate) fn parse_status(out: &[u8]) -> Result<WorkingStatus> {
    let mut st = WorkingStatus::default();
    let mut fields = out.split(|&b| b == 0).peekable();
    while let Some(entry) = fields.next() {
        if entry.is_empty() {
            continue;
        }
        let line = String::from_utf8_lossy(entry);
        let bad = || GitError::Parse(line.to_string());
        match entry[0] {
            b'#' => {
                let mut parts = line[2..].splitn(2, ' ');
                let key = parts.next().unwrap_or_default();
                let value = parts.next().unwrap_or_default();
                match key {
                    "branch.oid" if value != "(initial)" => st.head_oid = Some(value.into()),
                    "branch.head" if value != "(detached)" => st.branch = Some(value.into()),
                    "branch.upstream" => st.upstream = Some(value.into()),
                    "branch.ab" => {
                        let mut it = value.split(' ');
                        let a = it.next().and_then(|s| s[1..].parse().ok());
                        let b = it.next().and_then(|s| s[1..].parse().ok());
                        st.ahead_behind = AheadBehind {
                            ahead: a.unwrap_or(0),
                            behind: b.unwrap_or(0),
                        };
                    }
                    _ => {}
                }
            }
            b'1' => {
                // 1 XY sub mH mI mW hH hI path
                let parts: Vec<&str> = line.splitn(9, ' ').collect();
                let xy = parts.get(1).ok_or_else(bad)?.as_bytes();
                st.files.push(FileChange {
                    path: parts.get(8).ok_or_else(bad)?.to_string(),
                    old_path: None,
                    status: status_from_xy(xy[0], xy[1]),
                });
            }
            b'2' => {
                // 2 XY sub mH mI mW hH hI Xscore path \0 origPath
                let parts: Vec<&str> = line.splitn(10, ' ').collect();
                let xy = parts.get(1).ok_or_else(bad)?.as_bytes();
                let orig = fields
                    .next()
                    .map(|b| String::from_utf8_lossy(b).into_owned());
                let score = parts.get(8).ok_or_else(bad)?;
                st.files.push(FileChange {
                    path: parts.get(9).ok_or_else(bad)?.to_string(),
                    old_path: orig,
                    status: if score.starts_with('C') {
                        FileStatus::Copied
                    } else {
                        let s = status_from_xy(xy[0], xy[1]);
                        if s == FileStatus::Modified {
                            FileStatus::Renamed
                        } else {
                            s
                        }
                    },
                });
            }
            b'u' => {
                // u XY sub m1 m2 m3 mW h1 h2 h3 path
                let parts: Vec<&str> = line.splitn(11, ' ').collect();
                st.files.push(FileChange {
                    path: parts.get(10).ok_or_else(bad)?.to_string(),
                    old_path: None,
                    status: FileStatus::Conflicted,
                });
            }
            b'?' => st.files.push(FileChange {
                path: line[2..].to_string(),
                old_path: None,
                status: FileStatus::Untracked,
            }),
            _ => {}
        }
    }
    st.files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(st)
}
