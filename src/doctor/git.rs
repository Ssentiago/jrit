use super::CheckResult;
use crate::config::Config;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::{Command, Output};

pub(super) fn check_git_repository(root: &Path) -> CheckResult {
    match git(root, &["rev-parse", "--git-dir"]) {
        Ok(out) if out.status.success() => {
            CheckResult::ok("git repository", "the project is inside a git repository")
        }
        Ok(out) => CheckResult::fail(
            "git repository",
            format!("the project is not inside a git repository: {}", stderr(&out)),
        ),
        Err(e) => CheckResult::fail("git repository", format!("{e:#}")),
    }
}

pub(super) fn check_remote_origin(root: &Path) -> CheckResult {
    match git(root, &["remote", "get-url", "origin"]) {
        Ok(out) if out.status.success() => {
            CheckResult::ok("remote origin", format!("origin is {}", stdout(&out)))
        }
        _ => CheckResult::fail(
            "remote origin",
            "no 'origin' remote: jrit pushes commits and tags to origin",
        ),
    }
}

pub(super) fn check_branch(root: &Path, config: Option<&Config>) -> CheckResult {
    let out = match git(root, &["rev-parse", "--abbrev-ref", "HEAD"]) {
        Ok(out) if out.status.success() => out,
        Ok(out) => {
            return CheckResult::fail(
                "branch",
                format!("cannot determine the current branch: {}", stderr(&out)),
            );
        }
        Err(e) => {
            return CheckResult::fail(
                "branch",
                format!("cannot determine the current branch: {e:#}"),
            );
        }
    };
    let branch = stdout(&out);
    if branch == "HEAD" {
        return CheckResult::fail(
            "branch",
            "detached HEAD, check out a branch before releasing",
        );
    }
    match config {
        None => CheckResult::skipped(
            "branch",
            "cannot verify the allowed branches without a valid config",
        ),
        Some(config) if config.project.branches.iter().any(|b| b == &branch) => {
            CheckResult::ok("branch", format!("branch {branch} is allowed"))
        }
        Some(config) => CheckResult::fail(
            "branch",
            format!(
                "branch {branch} is not allowed for releases, switch to one of: {}",
                config.project.branches.join(", ")
            ),
        ),
    }
}

pub(super) fn check_working_tree(root: &Path) -> CheckResult {
    let out = match git(root, &["status", "--porcelain"]) {
        Ok(out) if out.status.success() => out,
        Ok(out) => {
            return CheckResult::fail(
                "working tree",
                format!("cannot read the working tree state: {}", stderr(&out)),
            );
        }
        Err(e) => {
            return CheckResult::fail(
                "working tree",
                format!("cannot read the working tree state: {e:#}"),
            );
        }
    };
    let entries: Vec<String> = stdout_lines(&out);
    if entries.is_empty() {
        return CheckResult::ok("working tree", "working tree is clean");
    }
    let total = entries.len();
    let noun = if total == 1 { "change" } else { "changes" };
    CheckResult::warning(
        "working tree",
        format!("{total} uncommitted {noun}:\n{}", capped(&entries, "\n")),
    )
}

pub(super) fn check_sync_with_origin(root: &Path) -> CheckResult {
    let branch_out = match git(root, &["rev-parse", "--abbrev-ref", "HEAD"]) {
        Ok(out) if out.status.success() => out,
        _ => {
            return CheckResult::skipped(
                "sync with origin",
                "cannot determine the current branch",
            );
        }
    };
    let branch = stdout(&branch_out);
    if branch == "HEAD" {
        return CheckResult::skipped(
            "sync with origin",
            "detached HEAD, nothing to compare with origin",
        );
    }

    let remote = match git(root, &["ls-remote", "--heads", "origin", &branch]) {
        Ok(out) if out.status.success() => stdout(&out),
        Ok(out) => {
            return CheckResult::fail(
                "sync with origin",
                format!("cannot reach origin: {}", stderr(&out)),
            );
        }
        Err(e) => {
            return CheckResult::fail("sync with origin", format!("cannot reach origin: {e:#}"));
        }
    };
    let Some(remote_sha) = remote.split_whitespace().next() else {
        return CheckResult::ok(
            "sync with origin",
            format!("branch {branch} is not on origin yet"),
        );
    };

    let local_sha = match git(root, &["rev-parse", "HEAD"]) {
        Ok(out) if out.status.success() => stdout(&out),
        _ => return CheckResult::fail("sync with origin", "cannot resolve the local HEAD"),
    };
    if local_sha == remote_sha {
        return CheckResult::ok("sync with origin", "in sync with origin");
    }

    let have_remote = git(root, &["cat-file", "-e", &format!("{remote_sha}^{{commit}}")])
        .map(|out| out.status.success())
        .unwrap_or(false);
    if !have_remote {
        return CheckResult::fail(
            "sync with origin",
            "origin has commits missing locally, pull before releasing",
        );
    }

    match git(root, &["merge-base", "--is-ancestor", remote_sha, "HEAD"]) {
        Ok(out) if out.status.success() => {
            let count = git(root, &["rev-list", "--count", &format!("{remote_sha}..HEAD")])
                .map(|out| stdout(&out))
                .unwrap_or_default();
            let noun = if count == "1" { "commit" } else { "commits" };
            CheckResult::ok(
                "sync with origin",
                format!("ahead of origin by {count} {noun}, they will be pushed with the release"),
            )
        }
        Ok(out) if out.status.code() == Some(1) => CheckResult::fail(
            "sync with origin",
            "local branch and origin have diverged, pull before releasing",
        ),
        _ => CheckResult::fail("sync with origin", "cannot compare the local branch with origin"),
    }
}

pub(super) fn git(root: &Path, args: &[&str]) -> Result<Output> {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .with_context(|| format!("failed to run git {:?}", args))
}

pub(super) fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

pub(super) fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).trim().to_string()
}

pub(super) fn stdout_lines(out: &Output) -> Vec<String> {
    stdout(out)
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect()
}

pub(super) fn capped(items: &[String], sep: &str) -> String {
    let mut shown: Vec<String> = items.iter().take(5).cloned().collect();
    if items.len() > 5 {
        shown.push(format!("and {} more", items.len() - 5));
    }
    shown.join(sep)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::CheckStatus;
    use crate::doctor::test_helpers::{bare_origin, clone_repo, git_ok, init_repo, make_config};

    #[test]
    fn git_repository_check_requires_a_repo() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(check_git_repository(dir.path()).status, CheckStatus::Fail);
        init_repo(dir.path());
        assert_eq!(check_git_repository(dir.path()).status, CheckStatus::Ok);
    }

    #[test]
    fn remote_origin_check_requires_origin() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        assert_eq!(check_remote_origin(dir.path()).status, CheckStatus::Fail);

        git_ok(dir.path(), &["remote", "add", "origin", "somewhere"]);
        assert_eq!(check_remote_origin(dir.path()).status, CheckStatus::Ok);
    }

    #[test]
    fn branch_check_enforces_allowed_list_and_head() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());

        let allowed = make_config(vec!["main"], vec![]);
        assert_eq!(check_branch(dir.path(), Some(&allowed)).status, CheckStatus::Ok);

        let other = make_config(vec!["release"], vec![]);
        let refused = check_branch(dir.path(), Some(&other));
        assert_eq!(refused.status, CheckStatus::Fail);
        assert!(refused.message.contains("not allowed"));

        assert_eq!(check_branch(dir.path(), None).status, CheckStatus::Skipped);

        git_ok(dir.path(), &["checkout", "-q", "--detach"]);
        let detached = check_branch(dir.path(), Some(&allowed));
        assert_eq!(detached.status, CheckStatus::Fail);
        assert!(detached.message.contains("detached"));
    }

    #[test]
    fn working_tree_is_clean_without_dirt() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        assert_eq!(check_working_tree(dir.path()).status, CheckStatus::Ok);
    }

    #[test]
    fn working_tree_warns_on_untracked_files() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("junk.txt"), "junk").unwrap();

        let result = check_working_tree(dir.path());
        assert_eq!(result.status, CheckStatus::Warning);
        assert!(result.message.contains("junk.txt"));
    }

    #[test]
    fn working_tree_warns_on_modified_files() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("f.txt"), "2").unwrap();

        let result = check_working_tree(dir.path());
        assert_eq!(result.status, CheckStatus::Warning);
        assert!(result.message.contains("f.txt"));
    }

    #[test]
    fn sync_check_reports_in_sync_and_ahead() {
        let dir = tempfile::tempdir().unwrap();
        let origin = bare_origin(dir.path());
        let work = dir.path().join("work");
        std::fs::create_dir_all(&work).unwrap();
        init_repo(&work);
        git_ok(&work, &["remote", "add", "origin", origin.to_str().unwrap()]);
        git_ok(&work, &["push", "-q", "-u", "origin", "main"]);

        let synced = check_sync_with_origin(&work);
        assert_eq!(synced.status, CheckStatus::Ok);
        assert!(synced.message.contains("in sync"));

        std::fs::write(work.join("f.txt"), "2").unwrap();
        git_ok(&work, &["add", "."]);
        git_ok(&work, &["commit", "-qm", "feat: more"]);

        let ahead = check_sync_with_origin(&work);
        assert_eq!(ahead.status, CheckStatus::Ok);
        assert!(ahead.message.contains("ahead of origin by 1"));
    }

    #[test]
    fn sync_check_fails_when_origin_is_ahead() {
        let dir = tempfile::tempdir().unwrap();
        let origin = bare_origin(dir.path());
        let work = dir.path().join("work");
        std::fs::create_dir_all(&work).unwrap();
        init_repo(&work);
        git_ok(&work, &["remote", "add", "origin", origin.to_str().unwrap()]);
        git_ok(&work, &["push", "-q", "-u", "origin", "main"]);

        let work2 = clone_repo(dir.path(), &origin, "work2");
        std::fs::write(work2.join("f.txt"), "2").unwrap();
        git_ok(&work2, &["add", "."]);
        git_ok(&work2, &["commit", "-qm", "feat: newer"]);
        git_ok(&work2, &["push", "-q", "origin", "main"]);

        let result = check_sync_with_origin(&work);
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.message.contains("missing locally"));
    }

    #[test]
    fn sync_check_fails_on_diverged_branches() {
        let dir = tempfile::tempdir().unwrap();
        let origin = bare_origin(dir.path());
        let work = dir.path().join("work");
        std::fs::create_dir_all(&work).unwrap();
        init_repo(&work);
        git_ok(&work, &["remote", "add", "origin", origin.to_str().unwrap()]);
        git_ok(&work, &["push", "-q", "-u", "origin", "main"]);

        std::fs::write(work.join("local.txt"), "2").unwrap();
        git_ok(&work, &["add", "."]);
        git_ok(&work, &["commit", "-qm", "feat: local"]);

        let work2 = clone_repo(dir.path(), &origin, "work2");
        std::fs::write(work2.join("other.txt"), "2").unwrap();
        git_ok(&work2, &["add", "."]);
        git_ok(&work2, &["commit", "-qm", "feat: other"]);
        git_ok(&work2, &["push", "-q", "origin", "main"]);

        git_ok(&work, &["fetch", "-q", "origin"]);

        let result = check_sync_with_origin(&work);
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.message.contains("diverged"));
    }

    #[test]
    fn sync_check_accepts_branch_missing_on_origin() {
        let dir = tempfile::tempdir().unwrap();
        let origin = bare_origin(dir.path());
        let work = dir.path().join("work");
        std::fs::create_dir_all(&work).unwrap();
        init_repo(&work);
        git_ok(&work, &["remote", "add", "origin", origin.to_str().unwrap()]);

        let result = check_sync_with_origin(&work);
        assert_eq!(result.status, CheckStatus::Ok);
        assert!(result.message.contains("not on origin yet"));
    }
}
