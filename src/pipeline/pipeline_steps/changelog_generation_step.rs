use crate::config::ChangelogType;
use crate::pipeline::{Pipeline, PipelineStep, Rollback};
use super::interactive_step::Interactive;
use anyhow::{Context, Result, bail};
use git_cliff_core::{
    changelog::Changelog,
    commit::Commit,
    config::{ChangelogConfig, CommitParser, Config, GitConfig},
    release::Release,
    repo::Repository,
};
use regex::Regex;
use std::path::{Path, PathBuf};

pub struct ChangelogGen {
    root: PathBuf,
    repo_path: String,
    version: String,
    changelog_path: PathBuf,
    changelog_type: ChangelogType,
    original_content: Option<String>,
}

impl ChangelogGen {
    pub fn new(
        root: PathBuf,
        repo_path: String,
        version: String,
        changelog_path: PathBuf,
        changelog_type: ChangelogType,
    ) -> Self {
        Self {
            root,
            repo_path,
            version,
            changelog_path,
            original_content: None,
            changelog_type,
        }
    }

    fn execute(&mut self) -> Result<()> {
        self.original_content = if self.changelog_path.exists() {
            Some(std::fs::read_to_string(&self.changelog_path)?)
        } else {
            None
        };

        let draft = match self.changelog_type {
            ChangelogType::Conventional => {
                generate_conventional(&self.root, &self.version, &self.repo_path)?
            }
            ChangelogType::Raw => generate_raw(&self.root, &self.version)?,
            other => bail!("changelog_type {other:?} does not generate a changelog"),
        };

        let edited = open_in_editor(&draft)?;
        prepend_to_changelog(&edited, &self.changelog_path)?;
        Ok(())
    }

    fn rollback(&mut self) -> Result<()> {
        match &self.original_content {
            Some(original) => {
                std::fs::write(&self.changelog_path, original).with_context(|| {
                    format!("rollback failed for {}", self.changelog_path.display())
                })?;
            }
            None => {
                if self.changelog_path.exists() {
                    std::fs::remove_file(&self.changelog_path).with_context(|| {
                        format!(
                            "rollback failed: could not remove {}",
                            self.changelog_path.display()
                        )
                    })?;
                }
            }
        }
        Ok(())
    }

    pub fn build_step(mut self) -> PipelineStep {
        PipelineStep {
            name: format!("generate changelog for {}", self.version),
            run: Box::new(move |_ctx| {
                Box::pin(async move {
                    self.execute()?;
                    let rollback: Rollback = Box::new(move || self.rollback());
                    Ok(rollback)
                })
            }),
            silent: false,
        }
    }
}

pub fn step() -> PipelineStep {
    PipelineStep::new("generate changelog", false, |ctx| {
        Box::pin(async move {
            let config = ctx.config.as_ref().context("config not loaded")?;
            if !config.project.changelog_type.is_generated() {
                return Ok(Pipeline::no_rollback());
            }

            let root = ctx
                .root
                .as_ref()
                .context("project root not determined")?
                .clone();
            let version = ctx
                .release_version
                .as_ref()
                .context("release version not selected")?
                .clone();
            let changelog_path = match &config.project.changelog {
                Some(p) => root.join(p),
                None => bail!("changelog path is required when changelog_type is not 'none'"),
            };
            if config.project.repo.is_empty() {
                bail!("project.repo is required for changelog generation");
            }

            let gen_step = ChangelogGen::new(
                root,
                config.project.repo.clone(),
                version,
                changelog_path,
                config.project.changelog_type,
            )
            .build_step();
            ctx.pending_steps.push(gen_step);
            Ok(Pipeline::no_rollback())
        })
    })
}

fn previous_release_tag(root: &Path) -> Result<Option<String>> {
    let output = std::process::Command::new("git")
        .args(["tag", "-l", "--merged", "HEAD"])
        .current_dir(root)
        .output()
        .context("failed to list git tags")?;

    if !output.status.success() {
        bail!(
            "git tag failed with {}\nstderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let tags = String::from_utf8(output.stdout)?;
    Ok(tags
        .lines()
        .filter_map(|tag| Interactive::parse_version_tag(tag).map(|version| (version, tag)))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, tag)| tag.to_string()))
}

fn generate_conventional(root: &Path, version: &str, repo_path: &str) -> Result<String> {
    let repo = Repository::init(root.to_path_buf())?;

    let body_template = "\
{% set order = [\"Features\", \"Bug Fixes\", \"Refactoring\", \"Chores\"] %}\
{% for group_name in order %}\
{% set group_commits = commits | filter(attribute=\"group\", value=group_name) %}\
{% if group_commits %}\
### {{ group_name }}\n\
{% for commit in group_commits %}- {{ commit.message }} ([`{{ commit.id | truncate(length=7, end=\"\") }}`](https://github.com/__REPO__/commit/{{ commit.id }}))\n{% endfor %}\n\
{% endif %}\
{% endfor %}"
        .replace("__REPO__", repo_path);

    let config = Config {
        changelog: ChangelogConfig {
            header: None,
            body: body_template,
            footer: None,
            trim: true,
            ..Default::default()
        },
        git: GitConfig {
            conventional_commits: true,
            filter_unconventional: false,
            commit_parsers: vec![
                CommitParser {
                    message: Some(Regex::new("^feat").unwrap()),
                    group: Some("Features".into()),
                    ..Default::default()
                },
                CommitParser {
                    message: Some(Regex::new("^fix").unwrap()),
                    group: Some("Bug Fixes".into()),
                    ..Default::default()
                },
                CommitParser {
                    message: Some(Regex::new("^refactor").unwrap()),
                    group: Some("Refactoring".into()),
                    ..Default::default()
                },
                CommitParser {
                    message: Some(Regex::new("^chore").unwrap()),
                    group: Some("Chores".into()),
                    skip: Some(true),
                    ..Default::default()
                },
            ],
            ..Default::default()
        },
        remote: Default::default(),
        bump: Default::default(),
    };

    let range = previous_release_tag(root)?.map(|tag| format!("{tag}..HEAD"));

    let git_commits = repo.commits(range.as_deref(), None, None, false)?;
    let commits: Vec<Commit> = git_commits.iter().map(Commit::from).collect();

    let release = Release {
        version: Some(version.to_string()),
        commits,
        ..Default::default()
    };

    let changelog = Changelog::new(vec![release], config, range.as_deref())?;
    let mut out = Vec::new();
    changelog.generate(&mut out)?;

    let body = String::from_utf8(out)?;
    let date = chrono::Local::now().format("%Y-%m-%d");
    Ok(format!("## [{version}] - {date}\n\n{body}"))
}

fn generate_raw(root: &Path, version: &str) -> Result<String> {
    let range_arg = previous_release_tag(root)?
        .map(|tag| format!("{tag}..HEAD"))
        .unwrap_or_else(|| "HEAD".to_string());

    let output = std::process::Command::new("git")
        .args(["log", "--oneline", "--no-decorate", &range_arg])
        .current_dir(root)
        .output()
        .context("failed to run git log")?;

    let log = String::from_utf8(output.stdout)?;
    let date = chrono::Local::now().format("%Y-%m-%d");
    Ok(format!("## [{version}] - {date}\n\n{log}"))
}

pub fn open_in_editor(content: &str) -> Result<String> {
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());

    let mut parts = editor.split_whitespace();
    let program = parts
        .next()
        .context("EDITOR is empty or whitespace only")?;
    let args: Vec<&str> = parts.collect();

    let mut tmp = tempfile::NamedTempFile::new()?;
    std::io::Write::write_all(&mut tmp, content.as_bytes())?;
    let tmp_path = tmp.path();

    let status = std::process::Command::new(program)
        .args(&args)
        .arg(tmp_path)
        .status()
        .with_context(|| format!("failed to open editor: {editor}"))?;

    if !status.success() {
        bail!("editor exited with non-zero status");
    }

    Ok(std::fs::read_to_string(tmp_path)?)
}

fn prepend_to_changelog(content: &str, changelog_path: &Path) -> Result<()> {
    let existing = if changelog_path.exists() {
        std::fs::read_to_string(changelog_path)?
    } else {
        String::new()
    };

    let combined = if existing.trim().is_empty() {
        format!("{}\n", content.trim())
    } else {
        format!("{}\n\n{}", content.trim(), existing)
    };
    std::fs::write(changelog_path, combined)?;

    Ok(())
}

