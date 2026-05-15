use crate::pipeline::{AppContext, Pipeline};
use anyhow::bail;

use crate::pipeline::pipeline_steps::git_step::GitOps;
use crate::pipeline::pipeline_steps::interactive_step::{ConfirmAction, Interactive};
use crate::pipeline::pipeline_steps::parse_changelog_step::Changelog;
use crate::pipeline::pipeline_steps::versioning_step::{
    JsonVersionBumper, TomlVersionBumper, VersionBumper,
};
use crate::pipeline::pipeline_steps::{
    build_step, changelog_generation_step, get_root_step, git_step, github_step, parse_config_step,
};
use std::path::PathBuf;

pub async fn run_pipeline() -> anyhow::Result<()> {
    let mut pipeline = Pipeline::new();

    pipeline
        .step(
            "find root dir",
            |ctx| {
                Box::pin(async move {
                    ctx.root = Some(get_root_step::find_root()?);
                    Ok(Pipeline::no_rollback())
                })
            },
            true,
        )
        .add(parse_config_step::step())
        .step(
            "retrieve tags",
            |ctx| {
                Box::pin(async move {
                    ctx.tags = GitOps::retrieve_current_tags()?;
                    Ok(Pipeline::no_rollback())
                })
            },
            true,
        )
        .step(
            "select version",
            |ctx| {
                Box::pin(async move {
                    let version = loop {
                        let v = if let Some(latest) = ctx.tags.first() {
                            Interactive::version_menu(&ctx.tags, latest)
                        } else {
                            Interactive::input_version(&ctx.tags, "", true)
                        }?;
                        match Interactive::confirm_version(&v)? {
                            ConfirmAction::Yes => break v,
                            ConfirmAction::No => return Ok(Pipeline::no_rollback()),
                            ConfirmAction::Retry => {}
                        }
                    };
                    ctx.release_version = Some(version);
                    Ok(Pipeline::no_rollback())
                })
            },
            false,
        )
        .step(
            "generate changelog",
            |ctx| {
                Box::pin(async move {
                    let config = ctx.config.as_ref().unwrap();
                    if matches!(config.project.changelog_type.as_str(), "manual" | "none") {
                        return Ok(Pipeline::no_rollback());
                    }

                    let root = ctx.root.as_ref().unwrap();
                    let version = ctx.release_version.as_ref().unwrap();
                    let changelog_path = match &config.project.changelog {
                        Some(p) => root.join(p),
                        None => {
                            bail!("changelog path is required when changelog_type is not 'none'")
                        }
                    };

                    let step = changelog_generation_step::ChangelogGen::new(
                        root.clone(),
                        version.clone(),
                        changelog_path,
                        config.project.changelog_type.clone(),
                    )
                    .build_step();

                    ctx.pending_steps.push(step);
                    Ok(Pipeline::no_rollback())
                })
            },
            false,
        )
        .step(
            "parse changelog",
            |ctx| {
                Box::pin(async move {
                    let config = ctx.config.as_ref().unwrap();
                    if config.project.changelog_type == "none" {
                        return Ok(Pipeline::no_rollback());
                    }
                    let version = ctx.release_version.as_ref().unwrap();
                    let changelog_path = match &config.project.changelog {
                        Some(p) => p.parse()?,
                        None => {
                            bail!("changelog path is required when changelog_type is not 'none'")
                        }
                    };
                    ctx.changelog = Some(Changelog::parse(version, changelog_path)?);
                    Ok(Pipeline::no_rollback())
                })
            },
            false,
        )
        .step(
            "check branch",
            |ctx| {
                Box::pin(async move {
                    let config = ctx.config.as_ref().unwrap();
                    let branch = GitOps::current_branch()?;
                    let branch = if !config.project.branches.contains(&branch) {
                        let selected = Interactive::select_branch(&config.project.branches)?;
                        GitOps::checkout(selected)?;
                        selected.to_string()
                    } else {
                        branch
                    };
                    ctx.branch = Some(branch);
                    Ok(Pipeline::no_rollback())
                })
            },
            false,
        )
        .step(
            "init bumpers",
            |ctx| {
                Box::pin(async move {
                    let config = ctx.config.as_ref().unwrap();
                    let version = ctx.release_version.clone().unwrap();

                    let bumpers: Vec<Box<dyn VersionBumper>> = config
                        .components
                        .iter()
                        .flat_map(|c| {
                            c.version_files.iter().map(
                                |vf| -> anyhow::Result<Box<dyn VersionBumper>> {
                                    let path = PathBuf::from(&vf.file);
                                    let resolved = vf.resolved_path()?;
                                    let bumper: Box<dyn VersionBumper> =
                                        match path.extension().and_then(|e| e.to_str()) {
                                            Some("toml") => {
                                                Box::new(TomlVersionBumper::new(path, resolved))
                                            }
                                            Some("json") => {
                                                Box::new(JsonVersionBumper::new(path, resolved))
                                            }
                                            Some(ext) => bail!("Unknown version file type: {ext}"),
                                            None => bail!("No extension: {}", vf.file),
                                        };
                                    Ok(bumper)
                                },
                            )
                        })
                        .collect::<anyhow::Result<_>>()?;

                    ctx.bumped_files = config
                        .components
                        .iter()
                        .flat_map(|c| c.version_files.iter().map(|vf| PathBuf::from(&vf.file)))
                        .collect();

                    for bumper in bumpers {
                        ctx.pending_steps.push(bumper.into_step(version.clone()));
                    }

                    Ok(Pipeline::no_rollback())
                })
            },
            true,
        )
        .step(
            "build",
            |ctx| {
                Box::pin(async move {
                    let config = ctx.config.as_ref().unwrap();
                    if config.project.release_mode == "ci" {
                        return Ok(Pipeline::no_rollback());
                    }
                    let root = ctx.root.as_ref().unwrap();
                    let builder = build_step::Builder::new(config.clone(), root.clone())?;
                    ctx.artifacts = builder.build_all()?;
                    Ok(Pipeline::no_rollback())
                })
            },
            false,
        )
        .step(
            "commit and push",
            |ctx| {
                Box::pin(async move {
                    let version = ctx.release_version.as_ref().unwrap();
                    let branch = ctx.branch.as_ref().unwrap();

                    if let Some(ref cf) = ctx.config {
                        if cf.project.changelog_type != "none" {
                            if let Some(ref changelog) = cf.project.changelog {
                                let step = git_step::CommitStep::new(git_step::CommitParams {
                                    files: vec![PathBuf::from(changelog)],
                                    message: "chore: update changelog".to_string(),
                                    branch: branch.clone(),
                                })
                                .build_step();
                                ctx.pending_steps.push(step);
                            }
                        }
                    }

                    let step = git_step::CommitStep::new(git_step::CommitParams {
                        files: ctx.bumped_files.clone(),
                        message: format!("chore: update app version to {version}"),
                        branch: branch.clone(),
                    })
                    .build_step();
                    ctx.pending_steps.push(step);

                    Ok(Pipeline::no_rollback())
                })
            },
            false,
        )
        .step(
            "create tag",
            |ctx| {
                Box::pin(async move {
                    let version = ctx.release_version.as_ref().unwrap();
                    let step = git_step::TagStep::new(git_step::TagParams {
                        tag: version.clone(),
                    })
                    .build_step();
                    ctx.pending_steps.push(step);
                    Ok(Pipeline::no_rollback())
                })
            },
            false,
        )
        .step(
            "github release",
            |ctx| {
                Box::pin(async move {
                    let config = ctx.config.as_ref().unwrap();
                    if config.project.release_mode == "ci" {
                        return Ok(Pipeline::no_rollback());
                    }
                    let version = ctx.release_version.as_ref().unwrap();
                    let artifacts = std::mem::take(&mut ctx.artifacts);

                    let paths: Vec<PathBuf> = artifacts.into_iter().flat_map(|a| a.paths).collect();
                    let prev_tag = ctx.tags.first().map(|s| s.as_str());
                    let body = match ctx.changelog.as_ref() {
                        Some(cl) => github_step::build_release_body(
                            &cl.body,
                            prev_tag,
                            version,
                            &config.project.repo,
                        ),
                        None => String::new(),
                    };
                    let release_params = github_step::ReleaseParams {
                        name: config.project.name.clone(),
                        repo_name: config.project.repo_name(),
                        repo_owner: config.project.owner(),
                        tag: version.clone(),
                        draft: false,
                        prerelease: false,
                        artifacts: paths,
                        body,
                    };

                    github_step::create_release(release_params).await?;
                    Ok(Pipeline::no_rollback())
                })
            },
            false,
        );

    let mut ctx = AppContext::new();
    pipeline.run(&mut ctx).await?;

    Ok(())
}
