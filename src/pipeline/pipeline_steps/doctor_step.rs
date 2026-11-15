use crate::doctor;
use crate::pipeline::{Pipeline, PipelineStep};
use anyhow::{Context, bail};
use owo_colors::OwoColorize;

/// Runs the preflight checks before the release touches anything. A real run
/// stops on any failed check, a dry run only reports.
pub fn preflight_step() -> PipelineStep {
    PipelineStep::new("preflight checks", false, |ctx| {
        Box::pin(async move {
            let report = doctor::diagnose(ctx.dry_run).await;
            print!("{}", report.render());
            if ctx.dry_run {
                if report.has_failures() {
                    println!(
                        "{} problems above do not stop a dry run",
                        "!".yellow().bold()
                    );
                }
                return Ok(Pipeline::no_rollback());
            }
            if report.has_failures() {
                bail!(
                    "{} of {} preflight checks failed, fix the issues above",
                    report.failure_count(),
                    report.checks.len()
                );
            }
            Ok(Pipeline::no_rollback())
        })
    })
}

/// Verifies the picked release tag is free on local and remote before the
/// pipeline starts changing anything.
pub fn verify_tag_step() -> PipelineStep {
    PipelineStep::new("verify tag is available", false, |ctx| {
        Box::pin(async move {
            let root = ctx.root.clone().context("project root not determined")?;
            let version = ctx
                .release_version
                .clone()
                .context("release version not selected")?;
            let result = doctor::check_tag_available(&root, &version, &ctx.tags);
            if result.status != doctor::CheckStatus::Ok {
                bail!("{}", result.message);
            }
            Ok(Pipeline::no_rollback())
        })
    })
}
