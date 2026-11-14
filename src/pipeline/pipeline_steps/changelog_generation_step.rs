use crate::config::ChangelogType;
use crate::pipeline::release_context;
use crate::pipeline::{Pipeline, PipelineStep, Rollback};
use anyhow::{Context, Result, bail};
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
            changelog_type,
            original_content: None,
        }
    }

    fn execute(&mut self) -> Result<()> {
        self.original_content = if self.changelog_path.exists() {
            Some(std::fs::read_to_string(&self.changelog_path)?)
        } else {
            None
        };

        let draft = release_context::render_draft(
            &self.root,
            &self.version,
            self.changelog_type,
            &self.repo_path,
        )?;

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
