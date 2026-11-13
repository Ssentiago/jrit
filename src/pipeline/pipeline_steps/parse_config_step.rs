use crate::config;
use crate::pipeline::{Pipeline, PipelineStep};

pub fn step() -> PipelineStep {
    PipelineStep {
        name: "parse config".into(),
        run: Box::new(|ctx| {
            Box::pin(async move {
                let root = ctx.root.as_ref().unwrap();
                ctx.config = Some(config::load(root.join("jrit.toml"))?);
                Ok(Pipeline::no_rollback())
            })
        }),
        silent: true,
    }
}
