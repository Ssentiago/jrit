use crate::config::ChangelogType;
use crate::pipeline::{Pipeline, PipelineStep};
use anyhow::Context;
use anyhow::bail;
use fancy_regex::Regex;
use fancy_regex::escape;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Changelog {
    pub version: String,
    pub body: String,
}

impl Changelog {
    pub fn parse(version: &str, path: PathBuf) -> anyhow::Result<Changelog> {
        if !path.exists() {
            bail!("Changelog at path {} not found", path.display());
        }

        let content = std::fs::read_to_string(&path)?;

        let escaped_version = escape(version);
        let pattern = Regex::new(&format!(
            r"(?ms)^## \[{version}\](?:\s*-\s*\d{{4}}-\d{{2}}-\d{{2}})?(.*?)(?=^## |\z)",
            version = escaped_version
        ))?;

        let regex_match = pattern.captures(&content)?;

        let body = regex_match
            .and_then(|m| m.get(1))
            .map(|m| m.as_str().trim())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "✗ changelog section for {} not found in {}\n  add the following to your changelog and re-run:\n  ## [{}]\n  ### Features\n  - ...",
                    version,
                    path.display(),
                    version
                )
            })?;

        anyhow::Ok(Self {
            version: version.to_string(),
            body: body.to_string(),
        })
    }
}

pub fn step() -> PipelineStep {
    PipelineStep::new("parse changelog", false, |ctx| {
        Box::pin(async move {
            let config = ctx.config.as_ref().context("config not loaded")?;
            if config.project.changelog_type == ChangelogType::None {
                return Ok(Pipeline::no_rollback());
            }

            let version = ctx
                .release_version
                .as_ref()
                .context("release version not selected")?;
            let root = ctx.root.as_ref().context("project root not determined")?;
            let changelog_path = match &config.project.changelog {
                Some(p) => root.join(p),
                None => bail!("changelog path is required when changelog_type is not 'none'"),
            };

            ctx.changelog = Some(Changelog::parse(version, changelog_path)?);
            Ok(Pipeline::no_rollback())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_changelog(dir: &std::path::Path, content: &str) -> PathBuf {
        let path = dir.join("CHANGELOG.md");
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn parse_extracts_only_target_version_section() {
        let dir = tempfile::tempdir().unwrap();
        let content = "# Changelog\n\n## [1.2.0] - 2024-05-01\n\n### Features\n- add thing\n\n## [1.1.0] - 2024-04-01\n\n### Fixes\n- fix thing\n";
        let path = write_changelog(dir.path(), content);

        let cl = Changelog::parse("1.2.0", path).unwrap();
        assert_eq!(cl.version, "1.2.0");
        assert!(cl.body.contains("add thing"));
        assert!(!cl.body.contains("fix thing"));
    }

    #[test]
    fn parse_missing_section_errors() {
        let dir = tempfile::tempdir().unwrap();
        let content = "# Changelog\n\n## [1.1.0] - 2024-04-01\n\n- old\n";
        let path = write_changelog(dir.path(), content);
        assert!(Changelog::parse("9.9.9", path).is_err());
    }

    #[test]
    fn parse_version_without_date() {
        let dir = tempfile::tempdir().unwrap();
        let content = "## [2.0.0]\n\n- something\n";
        let path = write_changelog(dir.path(), content);
        let cl = Changelog::parse("2.0.0", path).unwrap();
        assert!(cl.body.contains("something"));
    }
}
