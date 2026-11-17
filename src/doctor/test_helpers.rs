use crate::config::{ChangelogType, Component, Config, Project, ReleaseMode, VersionFile};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub(super) fn run(root: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap()
}

pub(super) fn git_ok(root: &Path, args: &[&str]) {
    let out = run(root, args);
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
}

pub(super) fn init_repo(root: &Path) {
    git_ok(root, &["init", "-q", "-b", "main"]);
    git_ok(root, &["config", "user.email", "t@t"]);
    git_ok(root, &["config", "user.name", "t"]);
    std::fs::write(root.join("f.txt"), "1").unwrap();
    git_ok(root, &["add", "."]);
    git_ok(root, &["commit", "-qm", "feat: first"]);
}

pub(super) fn bare_origin(dir: &Path) -> PathBuf {
    git_ok(dir, &["init", "-q", "--bare", "origin.git"]);
    let origin = dir.join("origin.git");
    git_ok(&origin, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    origin
}

pub(super) fn clone_repo(dir: &Path, origin: &Path, name: &str) -> PathBuf {
    git_ok(dir, &["clone", "-q", origin.to_str().unwrap(), name]);
    let work = dir.join(name);
    git_ok(&work, &["config", "user.email", "t@t"]);
    git_ok(&work, &["config", "user.name", "t"]);
    work
}

pub(super) fn make_config(branches: Vec<&str>, components: Vec<Component>) -> Config {
    Config {
        project: Project {
            name: "demo".into(),
            repo: "owner/demo".into(),
            branches: branches.into_iter().map(String::from).collect(),
            changelog_type: ChangelogType::None,
            changelog: None,
            release_mode: ReleaseMode::Local,
            sha256sums: false,
        },
        components,
    }
}

pub(super) fn demo_component(file: &str) -> Component {
    Component {
        name: "demo".into(),
        path: ".".into(),
        build: String::new(),
        artifact: String::new(),
        zip: true,
        version_files: vec![VersionFile {
            file: file.into(),
            path: None,
        }],
    }
}
