//! Manual smoke check of write operations on a scratch repo:
//!   cargo run -p gd-git --example smoke -- <scratch-dir-inside-project>
use gd_git::*;

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("scratch dir"));
    let _ = std::fs::remove_dir_all(&dir);
    let repo = Repository::init(&dir).unwrap();
    std::fs::write(dir.join("a.txt"), "a\n").unwrap();
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    let st = repo.status().unwrap();
    println!("unborn status: branch={:?} files={:?}", st.branch, st.files.iter().map(|f| (&f.path, f.status)).collect::<Vec<_>>());
    println!("untracked diff lines: {}", repo.working_diff(&st.files[0], Default::default()).unwrap().lines.len());
    // Partial commit: only a.txt.
    repo.commit("first", "body", &["a.txt".into()]).unwrap();
    let st = repo.status().unwrap();
    println!("after partial commit: {:?}", st.files.iter().map(|f| &f.path).collect::<Vec<_>>());
    repo.commit("second", "", &["b.txt".into()]).unwrap();
    std::fs::write(dir.join("a.txt"), "a\nmore\n").unwrap();
    let st = repo.status().unwrap();
    let d = repo.working_diff(&st.files[0], Default::default()).unwrap();
    println!("modified diff: +{} -{} lines={}", d.added, d.deleted, d.lines.len());
    // Partial commit: only the added line of a two-line change.
    std::fs::write(dir.join("a.txt"), "A\nmore\n").unwrap();
    let st = repo.status().unwrap();
    let d = repo.working_diff(&st.files[0], Default::default()).unwrap();
    let content = staged_text(&d, |ix| d.lines[ix].kind == DiffLineKind::Add && d.lines[ix].text == "more").unwrap();
    let pf = PartialFile { path: "a.txt".into(), old_path: None, content, source: d.source.clone().unwrap() };
    repo.commit_partial("partial", "", &[], &[pf]).unwrap();
    let show = std::process::Command::new("git").current_dir(&dir).args(["show", "HEAD:a.txt"]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&show.stdout), "a\nmore\n");
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "A\nmore\n");
    let st = repo.status().unwrap();
    println!("after line commit: HEAD has 'a, more'; still modified: {:?}", st.files.iter().map(|f| &f.path).collect::<Vec<_>>());
    repo.undo_last_commit().unwrap();
    let st = repo.status().unwrap();
    repo.discard(&st.files).unwrap();
    println!("after discard: {} changes", repo.status().unwrap().files.len());
    // Working diff is against HEAD even with part already staged, and re-editing
    // a modified file keeps the same status but must change the diff.
    {
        let git = |args: &[&str]| assert!(std::process::Command::new("git").current_dir(&dir).args(args).status().unwrap().success());
        std::fs::write(dir.join("a.txt"), "A\nstaged\n").unwrap();
        git(&["add", "a.txt"]);
        let before = repo.status().unwrap();
        std::fs::write(dir.join("a.txt"), "A\nstaged\nunstaged\n").unwrap();
        let after = repo.status().unwrap();
        assert_eq!(before.files, after.files, "status unchanged by re-edit");
        let d = repo.working_diff(&after.files[0], Default::default()).unwrap();
        assert_eq!(d.added, 3, "diff must include staged and unstaged changes vs HEAD");
        git(&["reset", "-q"]);
        repo.discard(&repo.status().unwrap().files).unwrap();
    }
    // ignore: literal + extension patterns, idempotent, ends with newline.
    std::fs::write(dir.join("junk.log"), "x").unwrap();
    std::fs::write(dir.join("we[ird].tmp"), "x").unwrap();
    repo.ignore(&[escape_ignore("we[ird].tmp"), "*.log".into()]).unwrap();
    repo.ignore(&["*.log".into()]).unwrap();
    println!(".gitignore: {:?}", std::fs::read_to_string(dir.join(".gitignore")).unwrap());
    let left: Vec<_> = repo.status().unwrap().files.into_iter().map(|f| f.path).collect();
    println!("after ignore: {left:?}");
    assert!(!left.iter().any(|p| p.ends_with(".log") || p.ends_with(".tmp")));
    repo.create_branch("feature/x").unwrap();
    println!("branches: {:?}", repo.branches().unwrap().iter().map(|b| (&b.name, b.is_head)).collect::<Vec<_>>());
    let log = repo.log(0, 10).unwrap();
    println!("log: {:?}", log.iter().map(|c| (&c.summary, &c.body)).collect::<Vec<_>>());
    println!("files in HEAD: {:?}", repo.commit_files(&log[0].sha).unwrap());
    println!("root commit diff: {:?}", repo.commit_diff(&log[1].sha, &repo.commit_files(&log[1].sha).unwrap()[0], Default::default()).unwrap().lines.len());
    repo.undo_last_commit().unwrap();
    println!("after undo: {:?}", repo.status().unwrap().files.iter().map(|f| (&f.path, f.status)).collect::<Vec<_>>());
    let main = repo.branches().unwrap().into_iter().find(|b| !b.is_head).unwrap();
    repo.checkout(&main).unwrap();
    println!("head now: {:?}", repo.status().unwrap().branch);
}
