use super::*;
use std::process::Command;

pub(crate) fn git(root: &Path, args: &[&str]) -> String {
    let configuration = test_fixture::GitTestConfig::new();
    let mut command = Command::new("git");
    configuration.configure(&mut command);
    let output = command.arg("-C").arg(root).args(args).output().unwrap();
    assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}

pub(crate) fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    git(directory.path(), &["init", "-q"]);
    directory
}

pub(crate) fn commit(root: &Path) {
    git(root, &["add", "--all"]);
    git(
        root,
        &[
            "-c",
            "user.name=Synthetic Git test",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ],
    );
}

fn key(root: &Path, path: &str, staged: bool) -> DiffKey {
    DiffKey { location: GitLocation::Local { root: root.into() }, path: path.into(), staged }
}

#[test]
fn git_diff_keeps_index_worktree_unicode_dash_paths_and_nested_cwd_separate() {
    let directory = repository();
    let root = directory.path();
    std::fs::create_dir(root.join("nested")).unwrap();
    for path in ["-name.rs", "nested/文件.rs"] {
        std::fs::write(root.join(path), "initial\n").unwrap();
    }
    commit(root);
    std::fs::write(root.join("-name.rs"), "index\n").unwrap();
    git(root, &["add", "--", "-name.rs"]);
    std::fs::write(root.join("-name.rs"), "worktree\n").unwrap();
    let staged = load(&key(root, "-name.rs", true)).unwrap();
    let worktree = load(&key(root, "-name.rs", false)).unwrap();
    assert!(staged.text.contains("+index") && !staged.text.contains("+worktree"));
    assert!(worktree.text.contains("-index") && worktree.text.contains("+worktree"));
    std::fs::write(root.join("nested/文件.rs"), "unicode change\n").unwrap();
    let nested = load(&key(&root.join("nested"), "nested/文件.rs", false)).unwrap();
    assert!(nested.text.contains("+unicode change"));
    assert_eq!(Path::new(&nested.root), root.canonicalize().unwrap());
    assert!(load(&key(root, "-name.rs", true)).unwrap().text == staged.text);
}

#[test]
fn git_diff_handles_rename_delete_untracked_binary_empty_and_clean_refresh() {
    let directory = repository();
    let root = directory.path();
    for path in ["old name.rs", "delete.rs", "clean.rs"] {
        std::fs::write(root.join(path), "initial\n").unwrap();
    }
    std::fs::write(root.join("binary.dat"), [0, 1, 2, 3]).unwrap();
    commit(root);
    git(root, &["mv", "--", "old name.rs", "new name.rs"]);
    let renamed = load(&key(root, "new name.rs", true)).unwrap();
    assert!(
        renamed.text.contains("rename from old name.rs")
            && renamed.text.contains("rename to new name.rs")
    );
    std::fs::remove_file(root.join("delete.rs")).unwrap();
    assert!(load(&key(root, "delete.rs", false)).unwrap().text.contains("deleted file mode"));
    std::fs::write(root.join("untracked 文件.rs"), "new contents\n").unwrap();
    let untracked = load(&key(root, "untracked 文件.rs", false)).unwrap();
    assert!(untracked.text.contains("new file mode"));
    assert!(untracked.text.contains("--- /dev/null"));
    assert!(untracked.text.contains("+new contents"));
    std::fs::write(root.join("empty.rs"), "").unwrap();
    assert!(load(&key(root, "empty.rs", false)).unwrap().text.contains("new file mode"));
    std::fs::write(root.join("-"), "literal dash contents\n").unwrap();
    assert!(load(&key(root, "-", false)).unwrap().text.contains("+literal dash contents"));
    std::fs::write(root.join("binary.dat"), [0, 2, 3, 4]).unwrap();
    assert!(load(&key(root, "binary.dat", false)).unwrap().text.contains("Binary files"));
    std::fs::write(root.join("clean.rs"), "change\n").unwrap();
    assert!(!load(&key(root, "clean.rs", false)).unwrap().text.is_empty());
    std::fs::write(root.join("clean.rs"), "initial\n").unwrap();
    assert!(load(&key(root, "clean.rs", false)).unwrap().text.is_empty());
}

#[test]
fn git_diff_rejects_outside_paths_and_caps_large_patches() {
    let directory = repository();
    let root = directory.path();
    for path in ["", "../outside", "/outside", "nested/../../outside", "nul\0name"] {
        assert!(load(&key(root, path, false)).is_err(), "accepted {path:?}");
    }
    std::fs::write(root.join("large.rs"), "synthetic line\n".repeat(150_000)).unwrap();
    let patch = load(&key(root, "large.rs", false)).unwrap();
    assert!(patch.truncated && patch.text.len() <= LIMIT + 3);
    assert!(!patch.lossy);
    let absent = tempfile::tempdir().unwrap();
    assert!(load(&key(absent.path(), "file.rs", false)).is_err());
}

#[test]
fn git_diff_conflict_is_a_read_only_combined_patch() {
    let directory = repository();
    let root = directory.path();
    std::fs::write(root.join("conflict.rs"), "base\n").unwrap();
    commit(root);
    let first = git(root, &["branch", "--show-current"]);
    git(root, &["checkout", "-qb", "incoming"]);
    std::fs::write(root.join("conflict.rs"), "incoming\n").unwrap();
    commit(root);
    git(root, &["checkout", "-q", first.trim()]);
    std::fs::write(root.join("conflict.rs"), "current\n").unwrap();
    commit(root);
    let configuration = test_fixture::GitTestConfig::new();
    let mut command = Command::new("git");
    configuration.configure(&mut command);
    let status = command.arg("-C").arg(root).args(["merge", "incoming"]).output().unwrap();
    assert!(!status.status.success());
    let before = std::fs::read(root.join("conflict.rs")).unwrap();
    let patch = load(&key(root, "conflict.rs", false)).unwrap();
    assert!(patch.text.contains("diff --cc conflict.rs"));
    assert_eq!(before, std::fs::read(root.join("conflict.rs")).unwrap());
    assert!(git(root, &["ls-files", "--unmerged"]).contains("conflict.rs"));
}
