use super::CheckResult;
use super::git::{capped, git, stderr, stdout, stdout_lines};
use crate::pipeline::release_context::parse_version_tag;
use anyhow::{Result, bail};
use std::path::Path;

pub(super) fn check_tags(root: &Path, dry_run: bool) -> CheckResult {
    if !dry_run {
        return match git(root, &["fetch", "origin", "--tags"]) {
            Ok(out) if out.status.success() => CheckResult::ok("tags", "tags fetched from origin"),
            Ok(out) => CheckResult::fail(
                "tags",
                format!("cannot fetch tags from origin: {}", stderr(&out)),
            ),
            Err(e) => CheckResult::fail("tags", format!("cannot fetch tags from origin: {e:#}")),
        };
    }

    let remote = match remote_tags(root) {
        Ok(tags) => tags,
        Err(e) => return CheckResult::fail("tags", format!("cannot reach origin: {e:#}")),
    };
    let local_out = match git(root, &["tag", "-l"]) {
        Ok(out) if out.status.success() => out,
        Ok(out) => {
            return CheckResult::fail(
                "tags",
                format!("cannot list local tags: {}", stderr(&out)),
            );
        }
        Err(e) => {
            return CheckResult::fail("tags", format!("cannot list local tags: {e:#}"));
        }
    };
    let local = stdout_lines(&local_out);
    let missing: Vec<String> = remote
        .into_iter()
        .filter(|tag| !local.contains(tag))
        .collect();

    if missing.is_empty() {
        CheckResult::ok("tags", "local tags match origin")
    } else {
        CheckResult::fail(
            "tags",
            format!(
                "local tags are missing {} from origin, run git fetch --tags",
                capped(&missing, ", ")
            ),
        )
    }
}

/// Verifies that a release tag is free both locally and on origin. Used by the
/// pipeline after the release version is picked.
pub fn check_tag_available(root: &Path, version: &str, local_tags: &[String]) -> CheckResult {
    let Some(parsed) = parse_version_tag(version) else {
        return CheckResult::fail(
            "tag availability",
            format!("'{version}' is not a valid semver version"),
        );
    };
    if local_tags
        .iter()
        .filter_map(|tag| parse_version_tag(tag))
        .any(|tag| tag == parsed)
    {
        return CheckResult::fail(
            "tag availability",
            format!("tag {version} already exists locally, pick another version"),
        );
    }
    match remote_tags(root) {
        Err(e) => CheckResult::fail(
            "tag availability",
            format!("cannot reach origin to verify the tag: {e:#}"),
        ),
        Ok(tags)
            if tags
                .iter()
                .filter_map(|tag| parse_version_tag(tag))
                .any(|tag| tag == parsed) =>
        {
            CheckResult::fail(
                "tag availability",
                format!("tag {version} already exists on origin, pick another version"),
            )
        }
        Ok(_) => CheckResult::ok("tag availability", format!("tag {version} is free")),
    }
}

fn remote_tags(root: &Path) -> Result<Vec<String>> {
    let out = git(root, &["ls-remote", "--tags", "origin"])?;
    if !out.status.success() {
        bail!("git ls-remote failed: {}", stderr(&out));
    }
    let mut tags = Vec::new();
    for line in stdout(&out).lines() {
        let Some((_, reference)) = line.split_once('\t') else {
            continue;
        };
        let Some(tag) = reference.strip_prefix("refs/tags/") else {
            continue;
        };
        if tag.ends_with("^{}") {
            continue;
        }
        tags.push(tag.to_string());
    }
    Ok(tags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::CheckStatus;
    use crate::doctor::test_helpers::{bare_origin, clone_repo, git_ok, run};
    use std::path::PathBuf;

    fn work_with_origin(dir: &Path) -> (PathBuf, PathBuf) {
        let origin = bare_origin(dir);
        let work = dir.join("work");
        std::fs::create_dir_all(&work).unwrap();
        git_ok(&work, &["init", "-q", "-b", "main"]);
        git_ok(&work, &["config", "user.email", "t@t"]);
        git_ok(&work, &["config", "user.name", "t"]);
        std::fs::write(work.join("f.txt"), "1").unwrap();
        git_ok(&work, &["add", "."]);
        git_ok(&work, &["commit", "-qm", "feat: first"]);
        git_ok(&work, &["remote", "add", "origin", origin.to_str().unwrap()]);
        git_ok(&work, &["push", "-q", "-u", "origin", "main"]);
        (origin, work)
    }

    #[test]
    fn tags_check_compares_with_origin_in_dry_run() {
        let dir = tempfile::tempdir().unwrap();
        let (origin, work) = work_with_origin(dir.path());

        git_ok(&work, &["tag", "v1.0.0"]);
        git_ok(&work, &["push", "-q", "origin", "v1.0.0"]);
        assert_eq!(check_tags(&work, true).status, CheckStatus::Ok);

        let work2 = clone_repo(dir.path(), &origin, "work2");
        git_ok(&work2, &["tag", "v2.0.0"]);
        git_ok(&work2, &["push", "-q", "origin", "v2.0.0"]);

        let stale = check_tags(&work, true);
        assert_eq!(stale.status, CheckStatus::Fail);
        assert!(stale.message.contains("v2.0.0"));
        assert!(stale.message.contains("git fetch --tags"));
    }

    #[test]
    fn tags_check_fetches_in_a_real_run() {
        let dir = tempfile::tempdir().unwrap();
        let (origin, work) = work_with_origin(dir.path());

        let work2 = clone_repo(dir.path(), &origin, "work2");
        git_ok(&work2, &["tag", "v2.0.0"]);
        git_ok(&work2, &["push", "-q", "origin", "v2.0.0"]);

        let fetched = check_tags(&work, false);
        assert_eq!(fetched.status, CheckStatus::Ok);
        let local = run(&work, &["tag", "-l"]);
        assert!(stdout(&local).contains("v2.0.0"));
    }

    #[test]
    fn tag_availability_detects_local_and_remote_collisions() {
        let dir = tempfile::tempdir().unwrap();
        let (_, work) = work_with_origin(dir.path());

        let free = check_tag_available(&work, "3.0.0", &[]);
        assert_eq!(free.status, CheckStatus::Ok);

        let local = check_tag_available(&work, "1.2.0", &["v1.2.0".to_string()]);
        assert_eq!(local.status, CheckStatus::Fail);
        assert!(local.message.contains("locally"));

        git_ok(&work, &["tag", "v2.0.0"]);
        git_ok(&work, &["push", "-q", "origin", "v2.0.0"]);
        git_ok(&work, &["tag", "-d", "v2.0.0"]);

        let remote = check_tag_available(&work, "2.0.0", &[]);
        assert_eq!(remote.status, CheckStatus::Fail);
        assert!(remote.message.contains("origin"));
    }
}
