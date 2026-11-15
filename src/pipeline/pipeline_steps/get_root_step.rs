use crate::pipeline::{Pipeline, PipelineStep};
use anyhow::{Context, anyhow};
use std::path::PathBuf;

pub fn find_root() -> anyhow::Result<PathBuf> {
    let mut current = std::env::current_dir().context("cannot determine current directory")?;
    loop {
        if current.join("jrit.toml").exists() {
            return Ok(current);
        }

        let Some(parent) = current.parent() else {
            return Err(anyhow!(
                "cannot find jrit.toml in this directory or any parent"
            ));
        };

        current = parent.to_path_buf();
    }
}

pub fn step() -> PipelineStep {
    PipelineStep::new("find root dir", true, |ctx| {
        Box::pin(async move {
            ctx.root = Some(find_root()?);
            Ok(Pipeline::no_rollback())
        })
    })
}
