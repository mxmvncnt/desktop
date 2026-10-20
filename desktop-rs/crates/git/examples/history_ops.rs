//! Exercises every History-tab git operation on scratch repos:
//!   cargo run -p gd-git --example history_ops -- <scratch-dir-inside-project>
//! Creates `<dir>/remote.git` (bare) and `<dir>/work` (clone) and asserts.
use std::path::Path;
use std::process::Command;

use gd_git::*;

fn sh(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").current_dir(dir).args(args).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn commit(dir: &Path, file: &str, content: &str, msg: &str) -> String {
    std::fs::write(dir.join(file), content).unwrap();
    sh(dir, &["add", "-A"]);
    sh(dir, &["commit", "-q", "-m", msg]);
    sh(dir, &["rev-parse", "HEAD"])
}

fn summaries(repo: &Repository) -> Vec<String> {
    repo.log(0, 50).unwrap().into_iter().map(|c| c.summary).collect()
}

fn main() {
    let base = std::path::PathBuf::from(std::env::args().nth(1).expect("scratch dir"));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let remote = base.join("remote.git");
    let work = base.join("work");
    sh(&base, &["init", "-q", "--bare", "-b", "main", "remote.git"]);
    sh(&base, &["clone", "-q", remote.to_str().unwrap(), "work"]);
    for (k, v) in [("user.name", "Tester"), ("user.email", "t@example.com")] {
        sh(&work, &["config", k, v]);
    }
    sh(&work, &["checkout", "-q", "-b", "main"]);
    let repo = Repository::open(&work).unwrap();

    // ---- push status ----
    assert!(repo.unpushed_commits().unwrap().is_empty(), "empty repo");
    for i in 1..=4 {
        commit(&work, &format!("f{i}.txt"), &format!("{i}\n"), &format!("c{i}"));
    }
    // No upstream yet: all commits are unpushed (remote exists, nothing on it).
    assert_eq!(repo.unpushed_commits().unwrap().len(), 4);
    repo.push("main", false).unwrap();
    assert!(repo.unpushed_commits().unwrap().is_empty());
    let c5 = commit(&work, "f5.txt", "5\n", "c5");
    let c6 = commit(&work, "f6.txt", "6\n", "c6");
    let un = repo.unpushed_commits().unwrap();
    assert!(un.contains(&c5) && un.contains(&c6) && un.len() == 2);
    println!("unpushed after 2 local commits: {} OK", un.len());
    // Unpublished branch (no upstream) off main: only commits not on any remote ref.
    sh(&work, &["checkout", "-q", "-b", "unpub"]);
    assert_eq!(repo.unpushed_commits().unwrap().len(), 2);
    sh(&work, &["checkout", "-q", "main"]);
    repo.push("main", true).unwrap();
    assert!(repo.unpushed_commits().unwrap().is_empty());
    println!("unpushed after push: 0 OK");
    println!("clone has no GitHub web url: {:?}", repo.github_web_url());

    // ---- web urls ----
    for (u, want) in [
        ("git@github.com:octo/hello.git", Some("https://github.com/octo/hello")),
        ("https://github.com/octo/hello", Some("https://github.com/octo/hello")),
        ("https://user:pw@github.com/octo/hello.git", Some("https://github.com/octo/hello")),
        ("ssh://git@github.com:22/octo/hello.git", Some("https://github.com/octo/hello")),
        ("git@github.example.org:team/x.git", Some("https://github.example.org/team/x")),
        ("git@gitlab.com:a/b.git", None),
        ("/home/me/remote.git", None),
    ] {
        assert_eq!(github_web_url(u).as_deref(), want, "{u}");
    }
    println!("github_web_url conversions OK");

    // ---- tags ----
    repo.create_tag("v1.0", &c5).unwrap();
    repo.create_tag("rc", &c5).unwrap();
    let tags = repo.tags_by_commit().unwrap();
    let mut t = tags[&c5].clone();
    t.sort();
    assert_eq!(t, ["rc", "v1.0"]);
    repo.delete_tag("rc").unwrap();
    assert_eq!(repo.tags_by_commit().unwrap()[&c5], ["v1.0"]);
    println!("tags create/list/delete OK");

    // ---- reset soft / mixed ----
    repo.reset_to_commit(&c5, ResetMode::Soft).unwrap();
    assert_eq!(sh(&work, &["status", "--porcelain"]), "A  f6.txt");
    sh(&work, &["reset", "-q", "--hard", &c6]);
    repo.reset_to_commit(&c5, ResetMode::Mixed).unwrap();
    assert_eq!(sh(&work, &["status", "--porcelain"]), "?? f6.txt");
    sh(&work, &["reset", "-q", "--hard", &c6]);
    println!("reset soft/mixed OK");

    // ---- checkout detached ----
    repo.checkout_commit(&c5).unwrap();
    assert_eq!(repo.status().unwrap().branch, None);
    sh(&work, &["checkout", "-q", "main"]);
    println!("checkout commit (detached) OK");

    // ---- create branch from commit ----
    repo.create_branch_at("from-c5", &c5).unwrap();
    assert_eq!(sh(&work, &["rev-parse", "HEAD"]), c5);
    sh(&work, &["checkout", "-q", "main"]);
    println!("create branch at commit OK");

    // ---- revert (clean + conflict) ----
    repo.revert_commit(&c6).unwrap();
    assert_eq!(summaries(&repo)[0], "Revert \"c6\"");
    assert!(!work.join("f6.txt").exists());
    sh(&work, &["reset", "-q", "--hard", &c6]);
    let conf = commit(&work, "f5.txt", "5 changed\n", "touch f5");
    commit(&work, "f5.txt", "5 changed again\n", "touch f5 again");
    let err = repo.revert_commit(&conf).unwrap_err();
    println!("revert conflict error: {}", err.to_string().lines().next().unwrap());
    assert_eq!(repo.history_op_in_progress(), Some("revert"));
    assert!(repo.revert_commit(&c6).is_err(), "ops blocked while in progress");
    repo.revert_abort().unwrap();
    assert_eq!(repo.history_op_in_progress(), None);
    sh(&work, &["reset", "-q", "--hard", &c6]);
    println!("revert OK (conflict left in progress, abort works)");

    // ---- cherry-pick (clean onto other branch + conflict) ----
    sh(&work, &["checkout", "-q", "-b", "other", &c5]);
    sh(&work, &["checkout", "-q", "main"]);
    repo.cherry_pick(&[c6.clone()], "other").unwrap();
    assert_eq!(repo.status().unwrap().branch.as_deref(), Some("other"));
    assert_eq!(summaries(&repo)[0], "c6");
    // conflict: main has f5 changed twice; other has a different f5.
    let o = commit(&work, "f5.txt", "other five\n", "other f5");
    let _ = o;
    sh(&work, &["checkout", "-q", "main"]);
    let c7 = commit(&work, "f5.txt", "main five\n", "main f5");
    sh(&work, &["checkout", "-q", "other"]);
    let err = repo.cherry_pick(&[c7.clone()], "other").unwrap_err();
    println!("cherry-pick conflict error: {}", err.to_string().lines().next().unwrap());
    assert_eq!(repo.history_op_in_progress(), Some("cherry-pick"));
    repo.cherry_pick_abort().unwrap();
    assert_eq!(repo.history_op_in_progress(), None);
    sh(&work, &["checkout", "-q", "main"]);
    sh(&work, &["reset", "-q", "--hard", &c6]);
    println!("cherry-pick OK");

    // ---- squash: tip run, middle run, errors ----
    sh(&work, &["checkout", "-q", "-B", "sq", &c6]);
    let d1 = commit(&work, "s1.txt", "1\n", "d1");
    let d2 = commit(&work, "s2.txt", "2\n", "d2");
    let d3 = commit(&work, "s3.txt", "3\n", "d3");
    let d4 = commit(&work, "s4.txt", "4\n", "d4");
    let tree_before = sh(&work, &["rev-parse", "HEAD^{tree}"]);
    std::fs::write(work.join("f1.txt"), "modified\n").unwrap();
    // Non-adjacent: refused.
    assert!(!repo.can_squash(&[d1.clone(), d3.clone()]));
    repo.squash_commits(&[d1.clone(), d3.clone()], "x").unwrap_err();
    // Tip run (d3 + d4), working tree untouched.
    repo.squash_commits(&[d4.clone(), d3.clone()], "d3+d4\n\nbody").unwrap();
    assert_eq!(&summaries(&repo)[..3], ["d3+d4", "d2", "d1"]);
    assert_eq!(sh(&work, &["rev-parse", "HEAD^{tree}"]), tree_before);
    assert_eq!(std::fs::read_to_string(work.join("f1.txt")).unwrap(), "modified\n");
    // Middle run (d1 + d2) with a commit above it.
    repo.squash_commits(&[d1.clone(), d2.clone()], "d1+d2").unwrap();
    assert_eq!(&summaries(&repo)[..3], ["d3+d4", "d1+d2", "c6"]);
    assert_eq!(sh(&work, &["rev-parse", "HEAD^{tree}"]), tree_before);
    assert_eq!(std::fs::read_to_string(work.join("f1.txt")).unwrap(), "modified\n", "autostash restored");
    assert_eq!(repo.history_op_in_progress(), None);
    println!("squash (tip + middle, dirty tree preserved) OK");
    sh(&work, &["checkout", "--", "f1.txt"]);

    // ---- reorder (move down / up) ----
    let before = summaries(&repo);
    let top = repo.log(0, 1).unwrap()[0].sha.clone();
    repo.move_commit(&top, false).unwrap();
    let after = summaries(&repo);
    assert_eq!(&after[..2], [before[1].as_str(), before[0].as_str()]);
    let top = repo.log(0, 2).unwrap()[1].sha.clone();
    repo.move_commit(&top, true).unwrap();
    assert_eq!(summaries(&repo)[..2], before[..2]);
    repo.move_commit(&repo.log(0, 1).unwrap()[0].sha, true).unwrap_err();
    assert!(!work.join(".git/gd-rebase-todo").exists());
    println!("reorder move down/up OK");

    // ---- reorder conflict stays in progress ----
    sh(&work, &["checkout", "-q", "-B", "ro", &c6]);
    commit(&work, "r.txt", "one\n", "r1");
    let r2 = commit(&work, "r.txt", "two\n", "r2");
    let err = repo.move_commit(&r2, false).unwrap_err();
    println!("reorder conflict error: {}", err.to_string().lines().next().unwrap());
    assert_eq!(repo.history_op_in_progress(), Some("rebase"));
    repo.rebase_abort().unwrap();
    assert_eq!(repo.history_op_in_progress(), None);
    println!("reorder conflict + rebase_abort OK");

    // ---- amend ----
    sh(&work, &["checkout", "-q", "main"]);
    std::fs::write(work.join("amended.txt"), "a\n").unwrap();
    let old = sh(&work, &["rev-parse", "HEAD"]);
    repo.amend_commit("reworded", "new body", &["amended.txt".into()]).unwrap();
    let log = repo.log(0, 2).unwrap();
    assert_eq!(log[0].summary, "reworded");
    assert_eq!(log[0].body, "new body");
    assert_ne!(log[0].sha, old);
    assert_eq!(log[0].parents, log[1..2].iter().map(|c| c.sha.clone()).collect::<Vec<_>>());
    assert!(repo.commit_files(&log[0].sha).unwrap().iter().any(|f| f.path == "amended.txt"));
    // Amending a pushed commit makes it unpushed-diverged: HEAD sha is in the unpushed set.
    assert!(repo.unpushed_commits().unwrap().contains(&log[0].sha));
    println!("amend OK");
    println!("ALL HISTORY OPS OK");
}
