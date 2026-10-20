//! Merge / squash / rebase / conflict smoke test on a scratch repo:
//!   cargo run -p gd-git --example merge_smoke -- <scratch-dir-inside-project>
//! Leaves the repository in a conflicted merge when the last argument is `--keep-conflict`
//! (used to prepare the `conflict-banner` snapshot repo).
use gd_git::*;
use std::path::Path;

fn sh(dir: &Path, args: &[&str]) {
    let st = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    assert!(st.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&st.stderr));
}

fn write_commit(dir: &Path, file: &str, text: &str, msg: &str) {
    std::fs::write(dir.join(file), text).unwrap();
    sh(dir, &["add", "-A"]);
    sh(dir, &["commit", "-qm", msg]);
}

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("scratch dir"));
    let keep = std::env::args().any(|a| a == "--keep-conflict");
    let _ = std::fs::remove_dir_all(&dir);
    let repo = Repository::init(&dir).unwrap();
    sh(&dir, &["checkout", "-q", "-b", "main"]);
    sh(&dir, &["config", "user.email", "t@example.com"]);
    sh(&dir, &["config", "user.name", "Tester"]);
    write_commit(&dir, "app.txt", "one\ntwo\nthree\n", "Initial commit");
    write_commit(&dir, "other.txt", "x\n", "Add other");
    assert_eq!(repo.in_progress(), None);

    // 1. Fast-forward merge.
    sh(&dir, &["checkout", "-q", "-b", "ff"]);
    write_commit(&dir, "ff.txt", "ff\n", "ff 1");
    write_commit(&dir, "ff.txt", "ff2\n", "ff 2");
    sh(&dir, &["checkout", "-q", "main"]);
    let p = repo.merge_preview("ff").unwrap();
    println!("preview ff: ahead={} behind={} conflicts={:?}", p.ahead, p.behind, p.conflicts);
    assert_eq!((p.ahead, p.behind), (2, 0));
    assert_eq!(repo.merge("ff").unwrap(), OpOutcome::Done);
    assert_eq!(repo.merge("ff").unwrap(), OpOutcome::UpToDate);
    println!("fast-forward merge OK");

    // 2. True merge (diverged, no conflicts).
    sh(&dir, &["checkout", "-q", "-b", "side"]);
    write_commit(&dir, "side.txt", "side\n", "side 1");
    sh(&dir, &["checkout", "-q", "main"]);
    write_commit(&dir, "main.txt", "main\n", "main 1");
    let p = repo.merge_preview("side").unwrap();
    assert_eq!((p.ahead, p.behind), (1, 1));
    assert_eq!(p.conflicts, Some(vec![]));
    assert_eq!(repo.merge("side").unwrap(), OpOutcome::Done);
    let head = repo.log(0, 1).unwrap();
    println!("true merge OK: {:?}", head[0].summary);
    assert!(head[0].summary.starts_with("Merge branch 'side'"));

    // 3. Squash merge.
    sh(&dir, &["checkout", "-q", "-b", "sq"]);
    write_commit(&dir, "sq1.txt", "1\n", "sq one");
    write_commit(&dir, "sq2.txt", "2\n", "sq two");
    sh(&dir, &["checkout", "-q", "main"]);
    let before = repo.log(0, 100).unwrap().len();
    assert_eq!(repo.merge_squash("sq").unwrap(), OpOutcome::Done);
    let log = repo.log(0, 100).unwrap();
    assert_eq!(log.len(), before + 1, "squash must add exactly one commit");
    assert_eq!(log[0].summary, "Squash merge sq into main");
    println!("squash OK: {:?} / {:?}", log[0].summary, log[0].body);

    // 4. Rebase current branch onto main.
    sh(&dir, &["checkout", "-q", "-b", "topic", "ff"]);
    write_commit(&dir, "topic.txt", "t\n", "topic 1");
    let p = repo.merge_preview("main").unwrap();
    println!("preview rebase onto main: ahead={} behind={}", p.ahead, p.behind);
    assert_eq!(repo.rebase("main").unwrap(), OpOutcome::Done);
    assert_eq!(repo.commits_ahead("main").unwrap(), 0);
    assert_eq!(repo.rebase("main").unwrap(), OpOutcome::UpToDate);
    println!("rebase OK");
    sh(&dir, &["checkout", "-q", "main"]);

    // 5. Conflicting merge: detection, status, ours/theirs, continue.
    sh(&dir, &["checkout", "-q", "-b", "c1"]);
    write_commit(&dir, "app.txt", "one\nFROM-C1\nthree\n", "c1 edits app");
    write_commit(&dir, "other.txt", "c1 other\n", "c1 edits other");
    sh(&dir, &["checkout", "-q", "main"]);
    write_commit(&dir, "app.txt", "one\nFROM-MAIN\nthree\n", "main edits app");
    write_commit(&dir, "other.txt", "main other\n", "main edits other");
    let p = repo.merge_preview("c1").unwrap();
    println!("preview c1: conflicts={:?}", p.conflicts);
    assert_eq!(p.conflicts.as_ref().map(|c| c.len()), Some(2));
    assert_eq!(repo.merge("c1").unwrap(), OpOutcome::Conflicts);
    let ip = repo.in_progress().expect("merge in progress");
    println!("in progress: {ip:?}");
    assert!(matches!(&ip, InProgress::Merge { branch: Some(b), .. } if b == "c1"));
    let st = repo.status().unwrap();
    let conflicted: Vec<_> = st.files.iter().filter(|f| f.status == FileStatus::Conflicted).map(|f| f.path.as_str()).collect();
    println!("conflicted: {conflicted:?}");
    assert_eq!(conflicted, ["app.txt", "other.txt"]);
    assert_eq!(st.files[0].status, FileStatus::Conflicted, "conflicts sort first");
    let d = repo.working_diff(&st.files[0], Default::default()).unwrap();
    assert!(d.lines.iter().any(|l| l.text.starts_with("<<<<<<<")), "conflict markers in diff");
    println!("prepared message: {:?}", repo.prepared_message());
    assert_ne!(repo.continue_operation(&ip).ok(), Some(OpOutcome::Done), "continue must not finish while conflicted");
    assert!(repo.in_progress().is_some());
    if keep {
        println!("left in conflicted merge state at {}", dir.display());
        return;
    }
    repo.resolve_with("app.txt", false).unwrap();
    repo.resolve_with("other.txt", true).unwrap();
    assert_eq!(std::fs::read_to_string(dir.join("app.txt")).unwrap(), "one\nFROM-MAIN\nthree\n");
    assert_eq!(std::fs::read_to_string(dir.join("other.txt")).unwrap(), "c1 other\n");
    assert!(!repo.has_unmerged());
    assert_eq!(repo.continue_operation(&ip).unwrap(), OpOutcome::Done);
    assert_eq!(repo.in_progress(), None);
    println!("use ours/theirs + merge --continue OK: {:?}", repo.log(0, 1).unwrap()[0].summary);

    // 6. Conflicting merge, abort.
    sh(&dir, &["checkout", "-q", "-b", "c2", "c1~2"]);
    write_commit(&dir, "app.txt", "one\nFROM-C2\nthree\n", "c2 edits app");
    sh(&dir, &["checkout", "-q", "main"]);
    let head_before = repo.log(0, 1).unwrap()[0].sha.clone();
    assert_eq!(repo.merge("c2").unwrap(), OpOutcome::Conflicts);
    let ip = repo.in_progress().unwrap();
    repo.abort_operation(&ip).unwrap();
    assert_eq!(repo.in_progress(), None);
    assert_eq!(repo.log(0, 1).unwrap()[0].sha, head_before);
    assert!(repo.status().unwrap().files.is_empty());
    println!("merge --abort OK");

    // 7. Conflicting squash, resolve with mark_resolved, continue.
    assert_eq!(repo.merge_squash("c2").unwrap(), OpOutcome::Conflicts);
    let ip = repo.in_progress().unwrap();
    assert_eq!(ip, InProgress::Squash);
    std::fs::write(dir.join("app.txt"), "one\nresolved\nthree\n").unwrap();
    repo.mark_resolved("app.txt").unwrap();
    assert_eq!(repo.continue_operation(&ip).unwrap(), OpOutcome::Done);
    assert_eq!(repo.in_progress(), None);
    println!("conflicting squash + continue OK");

    // 8. Conflicting rebase: pause, resolve, continue; and abort.
    sh(&dir, &["checkout", "-q", "-b", "r1", "c1~2"]);
    write_commit(&dir, "app.txt", "one\nFROM-R1\nthree\n", "r1 edits app");
    write_commit(&dir, "r1.txt", "r1\n", "r1 second");
    assert_eq!(repo.rebase("main").unwrap(), OpOutcome::Conflicts);
    let ip = repo.in_progress().unwrap();
    println!("in progress: {ip:?}");
    assert!(matches!(ip, InProgress::Rebase { step: 1, total: 2, .. }));
    repo.abort_operation(&ip).unwrap();
    assert_eq!(repo.in_progress(), None);
    assert_eq!(repo.rebase("main").unwrap(), OpOutcome::Conflicts);
    let ip = repo.in_progress().unwrap();
    repo.resolve_with("app.txt", true).unwrap();
    assert_eq!(repo.continue_operation(&ip).unwrap(), OpOutcome::Done);
    assert_eq!(repo.in_progress(), None);
    println!("rebase conflict abort + continue OK");
    sh(&dir, &["checkout", "-q", "main"]);

    // 9. Branch management.
    repo.create_branch_from("from-ff", "ff").unwrap();
    sh(&dir, &["checkout", "-q", "main"]);
    repo.rename_branch("from-ff", "renamed").unwrap();
    let names: Vec<_> = repo.branches().unwrap().into_iter().map(|b| b.name).collect();
    assert!(names.contains(&"renamed".to_string()) && !names.contains(&"from-ff".to_string()));
    repo.delete_branch("renamed", true).unwrap();
    println!("rename/create-from/delete OK");
    println!("ALL OK");
}
