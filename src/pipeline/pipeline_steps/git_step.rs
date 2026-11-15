use crate::config::ChangelogType;
use crate::pipeline::{Pipeline, PipelineStep, Rollback};
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;

pub struct CommitParams {
    pub files: Vec<PathBuf>,
    pub message: String,
    pub branch: String,
}

pub struct TagParams {
    pub tag: String,
}

pub struct GitOps;

impl GitOps {
    fn run(args: &[&str]) -> Result<()> {
        let status = Command::new("git")
            .args(args)
            .status()
            .with_context(|| format!("failed to spawn git {:?}", args))?;

        if !status.success() {
            anyhow::bail!("git {:?} failed with {}", args, status);
        }
        Ok(())
    }

    fn run_output(args: &[&str]) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .output()
            .with_context(|| format!("failed to spawn git {:?}", args))?;

        if !output.status.success() {
            anyhow::bail!(
                "git {:?} failed with {}\nstderr: {}",
                args,
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn has_staged_changes(files: &[&str]) -> Result<bool> {
        let mut args = vec!["diff", "--cached", "--quiet", "--"];
        args.extend_from_slice(files);
        let status = Command::new("git")
            .args(&args)
            .status()
            .with_context(|| format!("failed to spawn git {:?}", args))?;

        match status.code() {
            Some(0) => Ok(false),
            Some(1) => Ok(true),
            _ => anyhow::bail!("git {:?} failed with {}", args, status),
        }
    }

    pub fn retrieve_current_tags() -> Result<Vec<String>> {
        let output = Self::run_output(&["tag", "-l", "--sort=-version:refname"])?;
        let tags = output
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect();
        Ok(tags)
    }

    pub fn current_branch() -> Result<String> {
        Self::run_output(&["rev-parse", "--abbrev-ref", "HEAD"])
    }
}

pub struct CommitStep {
    pub params: CommitParams,
}

impl CommitStep {
    pub fn new(params: CommitParams) -> Self {
        Self { params }
    }

    pub fn build_step(self) -> PipelineStep {
        PipelineStep {
            name: format!("commit and push to {}", self.params.branch),
            run: Box::new(move |_ctx| {
                Box::pin(async move {
                    let branch = self.params.branch.clone();
                    let files: Vec<String> = self
                        .params
                        .files
                        .iter()
                        .map(|p| {
                            p.to_str()
                                .map(str::to_string)
                                .ok_or_else(|| {
                                    anyhow::anyhow!("path is not valid UTF-8: {}", p.display())
                                })
                        })
                        .collect::<anyhow::Result<_>>()?;
                    let file_refs: Vec<&str> = files.iter().map(String::as_str).collect();
                    let mut add_args = vec!["add", "--"];
                    add_args.extend(file_refs.iter().copied());
                    GitOps::run(&add_args)?;
                    if !GitOps::has_staged_changes(&file_refs)? {
                        return Ok(Pipeline::no_rollback());
                    }
                    let mut commit_args = vec!["commit", "-m", &self.params.message, "--"];
                    commit_args.extend(file_refs.iter().copied());
                    GitOps::run(&commit_args)?;
                    GitOps::run(&["push", "origin", &branch])?;
                    let commit = GitOps::run_output(&["rev-parse", "HEAD"])?;
                    let rollback: Rollback = Box::new(move || {
                        let head = GitOps::run_output(&["rev-parse", "HEAD"])?;
                        if head != commit {
                            anyhow::bail!("HEAD is {head}, expected {commit}; refusing to roll back");
                        }
                        let parent = format!("{commit}^");
                        GitOps::run(&["reset", "--soft", &parent])?;
                        let mut restore_args =
                            vec!["restore", "--source", &parent, "--staged", "--worktree", "--"];
                        restore_args.extend(files.iter().map(String::as_str));
                        GitOps::run(&restore_args)?;
                        GitOps::run(&["push", "--force-with-lease", "origin", &branch])?;
                        Ok(())
                    });
                    Ok(rollback)
                })
            }),
            silent: false,
        }
    }
}

pub struct TagStep {
    pub params: TagParams,
}

impl TagStep {
    pub fn new(params: TagParams) -> Self {
        Self { params }
    }

    pub fn build_step(self) -> PipelineStep {
        PipelineStep {
            name: format!("tag {}", self.params.tag),
            run: Box::new(move |_ctx| {
                Box::pin(async move {
                    let tag = self.params.tag.clone();
                    GitOps::run(&["tag", &tag])?;
                    GitOps::run(&["push", "origin", &tag])?;
                    let rollback: Rollback = Box::new(move || {
                        let _ = GitOps::run(&["tag", "-d", &tag]);
                        let _ = GitOps::run(&["push", "origin", "--delete", &tag]);
                        Ok(())
                    });
                    Ok(rollback)
                })
            }),
            silent: false,
        }
    }
}

pub fn retrieve_tags_step() -> PipelineStep {
    PipelineStep::new("retrieve tags", true, |ctx| {
        Box::pin(async move {
            ctx.tags = GitOps::retrieve_current_tags()?;
            Ok(Pipeline::no_rollback())
        })
    })
}

pub fn resolve_branch_step() -> PipelineStep {
    PipelineStep::new("resolve branch", true, |ctx| {
        Box::pin(async move {
            ctx.branch = Some(GitOps::current_branch()?);
            Ok(Pipeline::no_rollback())
        })
    })
}

pub fn commit_push_step() -> PipelineStep {
    PipelineStep::new("commit and push", false, |ctx| {
        Box::pin(async move {
            let version = ctx
                .release_version
                .as_ref()
                .context("release version not selected")?
                .clone();
            let branch = ctx.branch.as_ref().context("branch not resolved")?.clone();
            let root = ctx.root.clone().context("project root not determined")?;

            let mut pending: Vec<PipelineStep> = Vec::new();
            if let Some(config) = ctx.config.as_ref()
                && config.project.changelog_type != ChangelogType::None
                && let Some(changelog) = config.project.changelog.as_ref()
            {
                pending.push(
                    CommitStep::new(CommitParams {
                        files: vec![root.join(changelog)],
                        message: "chore: update changelog".to_string(),
                        branch: branch.clone(),
                    })
                    .build_step(),
                );
            }

            pending.push(
                CommitStep::new(CommitParams {
                    files: ctx.bumped_files.clone(),
                    message: format!("chore: update app version to {version}"),
                    branch,
                })
                .build_step(),
            );

            for step in pending {
                ctx.pending_steps.push(step);
            }
            Ok(Pipeline::no_rollback())
        })
    })
}

pub fn tag_push_step() -> PipelineStep {
    PipelineStep::new("create tag", false, |ctx| {
        Box::pin(async move {
            let version = ctx
                .release_version
                .as_ref()
                .context("release version not selected")?
                .clone();
            let step = TagStep::new(TagParams { tag: version }).build_step();
            ctx.pending_steps.push(step);
            Ok(Pipeline::no_rollback())
        })
    })
}
