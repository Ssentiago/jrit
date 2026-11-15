use super::CheckResult;
use crate::config::{ChangelogType, Config};
use crate::pipeline::pipeline_steps::versioning_step::VersionBumper;
use std::path::Path;

pub(super) fn check_version_files(root: &Path, config: &Config) -> CheckResult {
    let mut problems = Vec::new();
    for component in &config.components {
        for version_file in &component.version_files {
            let path = root.join(&component.path).join(&version_file.file);
            let display = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            let read = version_file
                .resolved_path()
                .and_then(|fields| VersionBumper::from_file(path.clone(), fields))
                .and_then(|bumper| bumper.current_version());
            if let Err(e) = read {
                problems.push(format!("{display}: {e:#}"));
            }
        }
    }

    if problems.is_empty() {
        CheckResult::ok(
            "version files",
            "every version file reads the current version",
        )
    } else {
        CheckResult::fail("version files", problems.join("\n"))
    }
}

pub(super) fn check_changelog(root: &Path, config: &Config) -> CheckResult {
    if config.project.changelog_type == ChangelogType::None {
        return CheckResult::skipped("changelog", "changelog is disabled in config");
    }
    let Some(name) = config.project.changelog.as_ref() else {
        return CheckResult::fail(
            "changelog",
            "changelog handling is enabled but no changelog file is configured",
        );
    };
    let path = root.join(name);
    if path.is_file() {
        CheckResult::ok("changelog", format!("{name} exists"))
    } else {
        CheckResult::fail("changelog", format!("{name} not found at {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::test_helpers::{demo_component, make_config};
    use crate::doctor::CheckStatus;

    #[test]
    fn version_files_read_current_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        let config = make_config(vec!["main"], vec![demo_component("Cargo.toml")]);

        let result = check_version_files(dir.path(), &config);
        assert_eq!(result.status, CheckStatus::Ok);
    }

    #[test]
    fn version_files_fail_on_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = make_config(vec!["main"], vec![demo_component("Cargo.toml")]);

        let result = check_version_files(dir.path(), &config);
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.message.contains("Cargo.toml"));
    }

    #[test]
    fn version_files_fail_on_missing_field() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();
        let config = make_config(vec!["main"], vec![demo_component("Cargo.toml")]);

        let result = check_version_files(dir.path(), &config);
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.message.contains("version"));
    }

    #[test]
    fn changelog_check_follows_config() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = make_config(vec!["main"], vec![]);
        assert_eq!(
            check_changelog(dir.path(), &config).status,
            CheckStatus::Skipped
        );

        config.project.changelog_type = ChangelogType::Conventional;
        config.project.changelog = Some("CHANGELOG.md".into());
        assert_eq!(check_changelog(dir.path(), &config).status, CheckStatus::Fail);

        std::fs::write(dir.path().join("CHANGELOG.md"), "# Changelog\n").unwrap();
        assert_eq!(check_changelog(dir.path(), &config).status, CheckStatus::Ok);
    }
}
