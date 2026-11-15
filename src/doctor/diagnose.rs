use super::report::{CheckResult, DoctorReport};
use super::{git, github, project, tags};
use crate::config::{self, ReleaseMode};
use crate::pipeline::pipeline_steps::get_root_step::find_root;

const CONFIG_REQUIRED: &str = "config is not loaded";

/// Runs every preflight check and collects the report. With dry_run set, the
/// tags check only compares local tags with origin and writes nothing.
pub async fn diagnose(dry_run: bool) -> DoctorReport {
    let mut checks = Vec::new();

    let root = match find_root() {
        Ok(root) => root,
        Err(e) => {
            checks.push(CheckResult::fail("config", format!("{e:#}")));
            return DoctorReport { checks };
        }
    };

    let config = match config::load(root.join("jrit.toml")) {
        Ok(config) => {
            checks.push(CheckResult::ok(
                "config",
                format!("jrit.toml at {}", root.display()),
            ));
            Some(config)
        }
        Err(e) => {
            checks.push(CheckResult::fail("config", format!("{e:#}")));
            None
        }
    };

    match &config {
        Some(config) => checks.push(project::check_version_files(&root, config)),
        None => checks.push(CheckResult::skipped("version files", CONFIG_REQUIRED)),
    }
    match &config {
        Some(config) => checks.push(project::check_changelog(&root, config)),
        None => checks.push(CheckResult::skipped("changelog", CONFIG_REQUIRED)),
    }

    checks.push(git::check_git_repository(&root));
    checks.push(git::check_remote_origin(&root));
    checks.push(git::check_branch(&root, config.as_ref()));
    checks.push(git::check_working_tree(&root));
    checks.push(git::check_sync_with_origin(&root));
    checks.push(tags::check_tags(&root, dry_run));

    match &config {
        Some(config) if config.project.release_mode == ReleaseMode::Ci => {
            checks.push(CheckResult::skipped(
                "github auth",
                "not needed in ci release mode",
            ));
        }
        Some(config) => checks.push(github::check_github_auth(config).await),
        None => checks.push(CheckResult::skipped("github auth", CONFIG_REQUIRED)),
    }

    DoctorReport { checks }
}
