use crate::pipeline::pipeline_steps::{
    build_step, changelog_generation_step, get_root_step, git_step, github_step, interactive_step,
    parse_changelog_step, parse_config_step, versioning_step,
};
use crate::pipeline::release_context;
use crate::pipeline::{AppContext, Pipeline};

pub async fn run_pipeline() -> anyhow::Result<()> {
    let mut pipeline = Pipeline::new();

    pipeline
        .add(get_root_step::step())
        .add(parse_config_step::step())
        .add(git_step::retrieve_tags_step())
        .add(release_context::collect_step())
        .add(interactive_step::select_version_step())
        .add(git_step::check_branch_step())
        .add(changelog_generation_step::step())
        .add(parse_changelog_step::step())
        .add(versioning_step::init_bumpers_step())
        .add(build_step::step())
        .add(git_step::commit_push_step())
        .add(git_step::tag_push_step())
        .add(github_step::release_step());

    let mut ctx = AppContext::new();
    pipeline.run(&mut ctx).await?;

    Ok(())
}
