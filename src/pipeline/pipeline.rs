use crate::config::Config;
use crate::pipeline::pipeline_steps::build_step::BuiltArtifact;
use crate::pipeline::pipeline_steps::parse_changelog_step::Changelog;
use crate::pipeline::release_context::ReleaseAnalysis;
use anyhow::Result;
use owo_colors::OwoColorize;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::pin::Pin;

pub type Rollback = Box<dyn FnOnce() -> Result<()>>;
pub type StepFuture<'a> = Pin<Box<dyn Future<Output = Result<Rollback>> + 'a>>;

#[derive(Debug)]
pub struct UserInterrupt;

impl std::fmt::Display for UserInterrupt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "interrupted by user")
    }
}

impl std::error::Error for UserInterrupt {}

pub struct PipelineStep {
    pub name: String,
    pub run: Box<dyn for<'a> FnOnce(&'a mut AppContext) -> StepFuture<'a>>,
    pub silent: bool,
}

impl PipelineStep {
    pub fn new(
        name: impl Into<String>,
        silent: bool,
        run: impl FnOnce(&mut AppContext) -> StepFuture + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            run: Box::new(run),
            silent,
        }
    }
}

pub struct AppContext {
    pub root: Option<PathBuf>,
    pub config: Option<Config>,
    pub tags: Vec<String>,
    pub release_version: Option<String>,
    pub release_analysis: Option<ReleaseAnalysis>,
    pub branch: Option<String>,
    pub changelog: Option<Changelog>,
    pub artifacts: Vec<BuiltArtifact>,
    pub pending_steps: Vec<PipelineStep>,
    pub bumped_files: Vec<PathBuf>,
    pub aborted: bool,
}

impl AppContext {
    pub fn new() -> Self {
        Self {
            root: None,
            config: None,
            tags: vec![],
            release_version: None,
            release_analysis: None,
            branch: None,
            changelog: None,
            artifacts: vec![],
            pending_steps: vec![],
            bumped_files: vec![],
            aborted: false,
        }
    }
}

impl Default for AppContext {
    fn default() -> Self {
        Self::new()
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
                    if ctx.aborted {
                        println!("{} Release aborted, reverting completed steps...", "!".yellow().bold());
                        Self::roll_back(rollbacks);
                        return Ok(());
                    }
                    if !ctx.pending_steps.is_empty() {
                        let pending = std::mem::take(&mut ctx.pending_steps);
                        for s in pending.into_iter().rev() {
                            self.steps.push_front(s);
                        }
                    }
                }
                Err(e) => {
                    if e.is::<UserInterrupt>() {
                        println!(
                            "{} Interrupted, reverting completed steps...",
                            "!".yellow().bold()
                        );
                        Self::roll_back(rollbacks);
                        return Ok(());
                    }
                    println!("{} {} — {e:#}", "✗".red(), step.name.red());
                    println!("{} Rolling back...", "!".yellow().bold());
                    Self::roll_back(rollbacks);
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    fn roll_back(rollbacks: Vec<(String, Rollback)>) {
        for (name, rb) in rollbacks.into_iter().rev() {
            println!("  {} {name}", "<-".yellow());
            if let Err(e) = rb() {
                eprintln!("  {} rollback failed for '{name}': {e:#}", "!".red());
            }
        }
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}
