use crate::pipeline::release_context::{
    self, bump_strings, parse_version_tag, BumpLevel, ReleaseAnalysis,
};
use crate::pipeline::{Pipeline, PipelineStep, UserInterrupt};
use anyhow::{Context, Result, bail};
use inquire::{InquireError, Select, Text};
use semver::Version;

pub struct Interactive;

impl Interactive {
    fn map_prompt<T>(result: std::result::Result<T, InquireError>) -> Result<T> {
        match result {
            Ok(v) => Ok(v),
            Err(InquireError::OperationInterrupted) => Err(UserInterrupt.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn pause() -> Result<()> {
        if let Err(e) = Text::new("Press Enter to continue...").prompt()
            && matches!(e, InquireError::OperationInterrupted)
        {
            return Err(UserInterrupt.into());
        }
        Ok(())
    }

    pub fn version_menu(
        tags: &[String],
        current: &str,
        analysis: &ReleaseAnalysis,
        draft: Option<&str>,
    ) -> Result<String> {
        let Some(parsed) = parse_version_tag(current) else {
            bail!("current tag '{current}' is not a valid semver version");
        };
        let (major, minor, patch) = (parsed.major, parsed.minor, parsed.patch);
        let [patch_v, minor_v, major_v] = bump_strings(major, minor, patch);

        print_context(analysis, current);

        let marker = "  (recommended)";
        let patch_s = match analysis.recommendation {
            Some(BumpLevel::Patch) => format!("Patch (bug fixes): {patch_v}{marker}"),
            _ => format!("Patch (bug fixes): {patch_v}"),
        };
        let minor_s = match analysis.recommendation {
            Some(BumpLevel::Minor) => format!("Minor (new features): {minor_v}{marker}"),
            _ => format!("Minor (new features): {minor_v}"),
        };
        let major_s = match analysis.recommendation {
            Some(BumpLevel::Major) => format!("Major (breaking changes): {major_v}{marker}"),
            _ => format!("Major (breaking changes): {major_v}"),
        };
        let manual_s = "Manual input".to_string();
        let since = analysis.prev_tag.as_deref().unwrap_or("the beginning");
        let changes_s = format!("View changes since {since}");
        let history_s = "View previous versions".to_string();
        let exit_s = "Exit".to_string();

        let options = vec![
            patch_s.as_str(),
            minor_s.as_str(),
            major_s.as_str(),
            changes_s.as_str(),
            manual_s.as_str(),
            history_s.as_str(),
            exit_s.as_str(),
        ];

        let starting_cursor = match analysis.recommendation {
            Some(BumpLevel::Major) => 2,
            Some(BumpLevel::Minor) => 1,
            Some(BumpLevel::Patch) | None => 0,
        };

        loop {
            let answer = Self::map_prompt(
                Select::new(
                    &format!("Current version: {current}. Select action:"),
                    options.clone(),
                )
                .with_starting_cursor(starting_cursor)
                .prompt(),
            )?;

            if answer == patch_s {
                return Ok(patch_v);
            }
            if answer == minor_s {
                return Ok(minor_v);
            }
            if answer == major_s {
                return Ok(major_v);
            }
            if answer == manual_s {
                return Self::input_version(tags, current, false);
            }
            if answer == exit_s {
                bail!("exit");
            }
            if answer == changes_s {
                show_changes(analysis, draft);
                Self::pause()?;
                continue;
            }

            if tags.is_empty() {
                println!("No previous versions.");
            } else {
                println!("Previous versions:\n  {}", tags.join("\n  "));
            }
            Self::pause()?;
        }
    }

    pub fn input_version(tags: &[String], current: &str, is_first: bool) -> Result<String> {
        let current_parsed = if is_first {
            None
        } else {
            parse_version_tag(current)
        };
        let tags = tags.to_vec();

        let version = Self::map_prompt(
            Text::new("Enter new version (semver) or leave empty to exit:")
                .with_validator(move |v: &str| {
                    let input = v.trim();
                    if input.is_empty() {
                        return Ok(inquire::validator::Validation::Valid);
                    }
                    let Ok(parsed) = Version::parse(input) else {
                        return Ok(inquire::validator::Validation::Invalid(
                            "Invalid semver format (e.g. 1.2.3)".into(),
                        ));
                    };
                    if tags
                        .iter()
                        .filter_map(|t| parse_version_tag(t))
                        .any(|t| t == parsed)
                    {
                        return Ok(inquire::validator::Validation::Invalid(
                            "Version already exists.".into(),
                        ));
                    }
                    if let Some(ref cur) = current_parsed
                        && &parsed <= cur
                    {
                        return Ok(inquire::validator::Validation::Invalid(
                            format!("Must be greater than current version {cur}").into(),
                        ));
                    }
                    Ok(inquire::validator::Validation::Valid)
                })
                .prompt(),
        )?;

        if version.trim().is_empty() {
            bail!("exit");
        }
        Ok(version.trim().to_string())
    }

    pub fn confirm_plan() -> Result<PlanAction> {
        let options = vec!["Apply", "Cancel"];
        let answer =
            Self::map_prompt(Select::new("Apply this release plan?", options).prompt())?;

        match answer {
            "Apply" => Ok(PlanAction::Apply),
            "Cancel" => Ok(PlanAction::Cancel),
            _ => bail!("unexpected answer"),
        }
    }

}

pub enum PlanAction {
    Apply,
    Cancel,
}

fn print_context(analysis: &ReleaseAnalysis, current: &str) {
    let since = analysis.prev_tag.as_deref().unwrap_or("the beginning");
    if analysis.is_empty() {
        println!("No commits since {since}. Nothing to release.");
        return;
    }
    println!(
        "Changes since {since} ({} commits): {} breaking, {} features, {} fixes, {} other.",
        analysis.commits.len(),
        analysis.breaking,
        analysis.feature,
        analysis.fix,
        analysis.other()
    );
    match analysis.recommendation {
        Some(level) => {
            let ver = analysis.recommended_version(current).unwrap_or_default();
            println!("Recommended: {} {}", level.label(), ver);
        }
        None if analysis.is_first_release() => {
            println!("First release: pick the starting version manually.");
        }
        None => {
            println!("No clear recommendation (commits are not conventional).");
        }
    }
}

fn show_changes(analysis: &ReleaseAnalysis, draft: Option<&str>) {
    let since = analysis.prev_tag.as_deref().unwrap_or("the beginning");
    if analysis.is_empty() {
        println!("No commits since {since}.");
        return;
    }
    println!("Commits since {since}:");
    for c in &analysis.commits {
        println!("  {}  [{}] {}", c.id, c.kind_label(), c.subject);
    }
    match draft {
        Some(d) => println!("\nDraft changelog:\n{d}"),
        None => println!("\n(changelog preview unavailable)"),
    }
}

pub fn select_version_step() -> PipelineStep {
    PipelineStep::new("select version", false, |ctx| {
        Box::pin(async move {
            let root = ctx
                .root
                .as_ref()
                .context("project root not determined")?
                .clone();
            let (changelog_type, repo_path) = {
                let config = ctx.config.as_ref().context("config not loaded")?;
                (config.project.changelog_type, config.project.repo.clone())
            };
            let analysis = ctx
                .release_analysis
                .take()
                .context("release analysis missing")?;

            let current = ctx
                .tags
                .iter()
                .find(|t| parse_version_tag(t).is_some())
                .cloned();

            let draft = match &current {
                Some(latest) if changelog_type.is_generated() => analysis
                    .preview_version(latest)
                    .and_then(|v| release_context::render_draft(&root, &v, changelog_type, &repo_path).ok()),
                _ => None,
            };

            let version = match &current {
                Some(latest) => {
                    Interactive::version_menu(&ctx.tags, latest, &analysis, draft.as_deref())?
                }
                None => {
                    show_changes(&analysis, None);
                    Interactive::input_version(&ctx.tags, "", true)?
                }
            };
            ctx.release_version = Some(version);
            Ok(Pipeline::no_rollback())
        })
    })
}
