//! Cross-check the in-process `similar` engine against git:
//!   cargo run --release -p gd-git --example diff_compare -- <repo> [commits]
//! For each file in the last N commits, compare engine (+added, -deleted)
//! with `git show --numstat`. Small differences are expected where several
//! equally-minimal diffs exist; large ones indicate a bug.

use std::time::Instant;

use gd_git::*;

fn main() {
    let mut args = std::env::args().skip(1);
    let repo = Repository::open(args.next().expect("repo path")).unwrap();
    let n: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(100);
    let (mut files, mut exact, mut close, mut binary) = (0, 0, 0, 0);
    let mut worst: Vec<(i64, String)> = Vec::new();
    let t = Instant::now();
    for c in repo.log(0, n).unwrap() {
        if c.parents.len() > 1 {
            continue;
        }
        let numstat = std::process::Command::new("git")
            .current_dir(repo.root())
            .args(["show", "--format=", "--numstat", "--no-renames", &c.sha])
            .output()
            .unwrap();
        let numstat = String::from_utf8_lossy(&numstat.stdout).to_string();
        for line in numstat.lines() {
            let f: Vec<&str> = line.splitn(3, '\t').collect();
            if f.len() < 3 || f[0] == "-" {
                binary += 1;
                continue;
            }
            let (ga, gd): (u32, u32) = (f[0].parse().unwrap(), f[1].parse().unwrap());
            let path = f[2].to_string();
            let status = if ga > 0 && gd == 0 && repo_missing(&repo, &c.sha, &path) {
                FileStatus::New
            } else {
                FileStatus::Modified
            };
            let fc = FileChange { path: path.clone(), old_path: None, status };
            let d = match repo.commit_diff(&c.sha, &fc, DiffOptions::default()) {
                Ok(d) => d,
                Err(e) => {
                    println!("ERR {path}: {e}");
                    continue;
                }
            };
            if d.is_binary || d.too_large {
                binary += 1;
                continue;
            }
            files += 1;
            let delta = (d.added as i64 - ga as i64).abs() + (d.deleted as i64 - gd as i64).abs();
            if delta == 0 {
                exact += 1;
            } else if delta <= 4 {
                close += 1;
            } else {
                worst.push((delta, format!("{} {path}: engine +{} -{} vs git +{ga} -{gd}", &c.sha[..8], d.added, d.deleted)));
            }
        }
    }
    worst.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, w) in worst.iter().take(10) {
        println!("DIFF {w}");
    }
    println!(
        "{files} files: {exact} exact, {close} within 4 lines, {} off by more; {binary} binary/skipped; {:?}",
        worst.len(),
        t.elapsed()
    );
}

fn repo_missing(repo: &Repository, sha: &str, path: &str) -> bool {
    std::process::Command::new("git")
        .current_dir(repo.root())
        .args(["cat-file", "-e", &format!("{sha}^1:{path}")])
        .status()
        .map(|s| !s.success())
        .unwrap_or(true)
}
