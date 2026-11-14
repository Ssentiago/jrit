use crate::config;
use crate::pipeline::{Pipeline, PipelineStep};
use anyhow::Context;

pub fn step() -> PipelineStep {
    PipelineStep::new("parse config", true, |ctx| {
        Box::pin(async move {
            let config_path = ctx
                .root
                .as_ref()
                .context("project root not determined")?
                .join("jrit.toml");
            ctx.config = Some(config::load(config_path)?);
            Ok(Pipeline::no_rollback())
        })
    })
}
