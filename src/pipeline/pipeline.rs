use crate::config::Config;
use crate::pipeline::pipeline_steps::build_step::BuiltArtifact;
use crate::pipeline::pipeline_steps::parse_changelog_step::Changelog;
use crate::pipeline::pipeline_steps::versioning_step::VersionBumper;
use anyhow::Result;
use owo_colors::OwoColorize;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::pin::Pin;

pub type Rollback = Box<dyn FnOnce() -> Result<()>>;
pub type StepFuture<'a> = Pin<Box<dyn Future<Output = Result<Rollback>> + 'a>>;

pub struct PipelineStep {
    pub name: String,
    pub run: Box<dyn for<'a> FnOnce(&'a mut AppContext) -> StepFuture<'a>>,
    pub silent: bool,
}

pub struct AppContext {
    pub root: Option<PathBuf>,
    pub config: Option<Config>,
    pub tags: Vec<String>,
    pub release_version: Option<String>,
    pub branch: Option<String>,
    pub changelog: Option<Changelog>,
    pub bumpers: Vec<Box<dyn VersionBumper>>,
    pub artifacts: Vec<BuiltArtifact>,
    pub pending_steps: Vec<PipelineStep>,
    pub bumped_files: Vec<PathBuf>,
}

impl AppContext {
    pub fn new() -> Self {
        Self {
            root: None,
            config: None,
            tags: vec![],
            release_version: None,
            branch: None,
            changelog: None,
            bumpers: vec![],
            artifacts: vec![],
            pending_steps: vec![],
            bumped_files: vec![],
        }
    }
}

pub struct Pipeline {
    steps: VecDeque<PipelineStep>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            steps: VecDeque::new(),
        }
    }

    pub fn no_rollback() -> Rollback {
        Box::new(|| Ok(()))
    }

    pub fn add(&mut self, step: PipelineStep) -> &mut Self {
        self.steps.push_back(step);
        self
    }

    pub fn step(
        &mut self,
        name: impl Into<String>,
        run: impl FnOnce(&mut AppContext) -> StepFuture + 'static,
        silent: bool,
    ) -> &mut Self {
        self.steps.push_back(PipelineStep {
            name: name.into(),
            run: Box::new(run),
            silent,
        });
        self
    }

    pub async fn run(&mut self, ctx: &mut AppContext) -> Result<()> {
        let mut rollbacks: Vec<(String, Rollback)> = vec![];

        while let Some(step) = self.steps.pop_front() {
            if !step.silent {
                println!("{} {}", "→".dimmed(), step.name.dimmed());
            }
            match (step.run)(ctx).await {
                Ok(rb) => {
                    if !step.silent {
                        println!("{} {}", "✓".green(), step.name.clone().green());
                    }
                    rollbacks.push((step.name, rb));
                    if !ctx.pending_steps.is_empty() {
                        let pending = std::mem::take(&mut ctx.pending_steps);
                        for s in pending.into_iter().rev() {
                            self.steps.push_front(s);
                        }
                    }
                }
                Err(e) => {
                    println!("{} {} — {e:#}", "✗".red(), step.name.red());
                    println!("{} Rolling back...", "!".yellow().bold());
                    for (name, rb) in rollbacks.into_iter().rev() {
                        println!("  {} {name}", "<-".yellow());
                        if let Err(e) = rb() {
                            eprintln!("  {} rollback failed for '{name}': {e:#}", "!".red());
                        }
                    }
                    return Err(e);
                }
            }
        }
        Ok(())
    }
}
