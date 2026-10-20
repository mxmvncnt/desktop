//! Smoke check of stash operations on a scratch repo:
//!   cargo run -p gd-git --example stash_smoke -- <scratch-dir-inside-project>
//! Leaves the repo behind (with several stashes) for snapshot renders:
//! pass `--keep` as a second argument to skip the destructive tail (apply/pop/drop).
use gd_git::*;

fn sh(dir: &std::path::Path, args: &[&str]) {
    let ok = std::process::Command::new("git").args(args).current_dir(dir).status().unwrap().success();
    assert!(ok, "git {args:?} failed");
}

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("scratch dir"));
    let keep = std::env::args().nth(2).as_deref() == Some("--keep");
    let _ = std::fs::remove_dir_all(&dir);
    let repo = Repository::init(&dir).unwrap();
    sh(&dir, &["config", "user.name", "Test"]);
    sh(&dir, &["config", "user.email", "t@example.com"]);
    sh(&dir, &["checkout", "-q", "-b", "main"]);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    for i in 1..=3 {
        std::fs::write(dir.join(format!("file{i}.txt")), format!("line {i}\n")).unwrap();
    }
    std::fs::write(dir.join("src/main.rs"), "fn main() {\n    println!(\"hello\");\n}\n").unwrap();
    repo.commit("init", "", &repo.status().unwrap().files.iter().map(|f| f.path.clone()).collect::<Vec<_>>()).unwrap();

    // 1. Everything on main: tracked edit + untracked file, with a message.
    std::fs::write(dir.join("file1.txt"), "line 1\nedited\n").unwrap();
    std::fs::write(dir.join("src/new.rs"), "pub fn added() {}\n").unwrap();
    let a = repo.stash_push("Half-done refactor", &[]).unwrap();
    assert!(repo.status().unwrap().files.is_empty(), "stash -u should clean the tree");

    // 2. WIP-style (no message) on main, tracked only.
    std::fs::write(dir.join("file2.txt"), "line 2\nchanged\n").unwrap();
    let b = repo.stash_push("", &[]).unwrap();

    // 3. Pathspec-only stash: two dirty files, stash just one (plus an untracked one).
    std::fs::write(dir.join("file3.txt"), "line 3\nmore\n").unwrap();
    std::fs::write(dir.join("src/main.rs"), "fn main() {\n    println!(\"goodbye\");\n}\n").unwrap();
    std::fs::write(dir.join("scratch notes [1].md"), "# notes\n").unwrap();
    let c = repo.stash_push("only main.rs + notes", &["src/main.rs".into(), "scratch notes [1].md".into()]).unwrap();
    let left: Vec<_> = repo.status().unwrap().files.into_iter().map(|f| f.path).collect();
    println!("after pathspec stash, left in tree: {left:?}");
    assert_eq!(left, vec!["file3.txt".to_string()]);
    repo.discard(&repo.status().unwrap().files).unwrap();

    // 4. Another branch.
    repo.create_branch("feature/x").unwrap();
    std::fs::write(dir.join("feature.txt"), "feature work\n").unwrap();
    let d = repo.stash_push("Feature WIP", &[]).unwrap();
    repo.create_branch("feature/y").unwrap();
    std::fs::write(dir.join("file1.txt"), "line 1\nother\n").unwrap();
    std::fs::create_dir_all(dir.join("dir with space")).unwrap();
    std::fs::write(dir.join("dir with space/f.txt"), "x\n").unwrap();
    let _e = repo.stash_push("", &[]).unwrap();
    sh(&dir, &["checkout", "-q", "main"]);

    let list = repo.stash_list().unwrap();
    for s in &list {
        println!("stash@{{{}}} {} branch={:?} msg={:?} title={:?} date={} t={}", s.index, s.short_sha, s.branch, s.message, s.title(), s.date, s.time);
    }
    assert_eq!(list.len(), 5);
    assert_eq!(list[0].branch.as_deref(), Some("feature/y"));
    assert_eq!(list[4].message, "Half-done refactor");
    assert_eq!(list[3].message, "");
    assert_eq!(list[3].branch.as_deref(), Some("main"));

    // Files + diffs, including the untracked file in the third parent.
    for (name, sha) in [("a", &a), ("b", &b), ("c", &c), ("d", &d)] {
        let files = repo.stash_files(sha).unwrap();
        println!("stash {name}: {:?}", files.iter().map(|f| (&f.path, f.status)).collect::<Vec<_>>());
        for f in &files {
            let diff = repo.stash_diff(sha, f).unwrap();
            println!("  {} +{} -{} lines={}", f.path, diff.added, diff.deleted, diff.lines.len());
            assert!(!diff.lines.is_empty(), "empty diff for {}", f.path);
        }
    }
    let af = repo.stash_files(&a).unwrap();
    assert!(af.iter().any(|f| f.path == "src/new.rs" && f.status == FileStatus::Untracked));
    assert!(af.iter().any(|f| f.path == "file1.txt" && f.status == FileStatus::Modified));
    assert_eq!(repo.stash_diff(&a, af.iter().find(|f| f.path == "src/new.rs").unwrap()).unwrap().added, 1);

    // Rename (moves to top of list).
    repo.stash_rename(&b, Some("main"), "Renamed WIP").unwrap();
    let list = repo.stash_list().unwrap();
    assert_eq!(list[0].sha, b);
    assert_eq!(list[0].message, "Renamed WIP");
    assert_eq!(list.len(), 5);

    if keep {
        println!("kept {} stashes", list.len());
        return;
    }

    // Apply keeps it, restores untracked too.
    repo.stash_apply(&a).unwrap();
    let st: Vec<_> = repo.status().unwrap().files.into_iter().map(|f| f.path).collect();
    println!("after apply a: {st:?}");
    assert!(st.contains(&"src/new.rs".to_string()) && st.contains(&"file1.txt".to_string()));
    assert_eq!(repo.stash_list().unwrap().len(), 5);
    repo.discard(&repo.status().unwrap().files).unwrap();

    // Pop removes exactly that one even though it is not on top.
    repo.stash_pop(&c).unwrap();
    let list = repo.stash_list().unwrap();
    assert_eq!(list.len(), 4);
    assert!(!list.iter().any(|s| s.sha == c));
    assert!(dir.join("scratch notes [1].md").exists());
    repo.discard(&repo.status().unwrap().files).unwrap();

    // Conflict: stash edits file1, then commit a different edit to file1 and pop.
    std::fs::write(dir.join("file1.txt"), "line 1\nstash side\n").unwrap();
    let f = repo.stash_push("will conflict", &[]).unwrap();
    std::fs::write(dir.join("file1.txt"), "line 1\ncommitted side\n").unwrap();
    repo.commit("diverge", "", &["file1.txt".into()]).unwrap();
    let err = repo.stash_pop(&f).unwrap_err();
    println!("pop conflict error: {err}");
    assert!(err.to_string().contains("conflict"));
    assert!(repo.stash_list().unwrap().iter().any(|s| s.sha == f), "stash must survive a conflicted pop");
    println!("status after conflict: {:?}", repo.status().unwrap().files.iter().map(|f| (&f.path, f.status)).collect::<Vec<_>>());

    // Drop.
    let before = repo.stash_list().unwrap().len();
    repo.stash_drop(&d).unwrap();
    let list = repo.stash_list().unwrap();
    assert_eq!(list.len(), before - 1);
    assert!(!list.iter().any(|s| s.sha == d));
    assert!(repo.stash_drop(&d).is_err());
    println!("stash smoke OK ({} left)", list.len());
}
