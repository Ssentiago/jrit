use crate::pipeline::{Pipeline, PipelineStep, UserInterrupt};
use anyhow::{Result, bail};
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

    pub fn parse_version_tag(tag: &str) -> Option<Version> {
        let trimmed = tag.trim();
        let stripped = trimmed
            .strip_prefix('v')
            .or_else(|| trimmed.strip_prefix('V'))
            .unwrap_or(trimmed);
        Version::parse(stripped).ok()
    }

    pub fn version_menu(tags: &[String], current: &str) -> Result<String> {
        let Some(parsed) = Self::parse_version_tag(current) else {
            bail!("current tag '{current}' is not a valid semver version");
        };
        let (major, minor, patch) = (parsed.major, parsed.minor, parsed.patch);

        let patch_s = format!("Patch (bug fixes): {major}.{minor}.{}", patch + 1);
        let minor_s = format!("Minor (new features): {major}.{}.0", minor + 1);
        let major_s = format!("Major (breaking changes): {}.0.0", major + 1);
        let manual_s = "Manual input".to_string();
        let history_s = "View previous versions".to_string();
        let exit_s = "Exit".to_string();

        let options = vec![
            patch_s.as_str(),
            minor_s.as_str(),
            major_s.as_str(),
            manual_s.as_str(),
            history_s.as_str(),
            exit_s.as_str(),
        ];

        loop {
            let answer = Self::map_prompt(
                Select::new(
                    &format!("Current version: {current}. Select action:"),
                    options.clone(),
                )
                .prompt(),
            )?;

            if answer == patch_s {
                return Ok(format!("{major}.{minor}.{}", patch + 1));
            }
            if answer == minor_s {
                return Ok(format!("{major}.{}.0", minor + 1));
            }
            if answer == major_s {
                return Ok(format!("{}.0.0", major + 1));
            }
            if answer == manual_s {
                return Self::input_version(tags, current, false);
            }
            if answer == exit_s {
                bail!("exit");
            }

            if tags.is_empty() {
                println!("No previous versions.");
            } else {
                println!("Previous versions:\n  {}", tags.join("\n  "));
            }
            if let Err(e) = Text::new("Press Enter to continue...").prompt()
                && matches!(e, InquireError::OperationInterrupted)
            {
                return Err(UserInterrupt.into());
            }
        }
    }

    pub fn input_version(tags: &[String], current: &str, is_first: bool) -> Result<String> {
        let current_parsed = if is_first {
            None
        } else {
            Self::parse_version_tag(current)
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
                        .filter_map(|t| Self::parse_version_tag(t))
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

    pub fn confirm_version(version: &str) -> Result<ConfirmAction> {
        let options = vec!["Yes", "No", "Retry"];
        let answer = Self::map_prompt(
            Select::new(&format!("Selected version: {version}. Continue?"), options).prompt(),
        )?;

        match answer {
            "Yes" => Ok(ConfirmAction::Yes),
            "No" => Ok(ConfirmAction::No),
            "Retry" => Ok(ConfirmAction::Retry),
            _ => bail!("unexpected answer"),
        }
    }

    pub fn select_branch(allowed: &[String]) -> Result<&str> {
        let opts: Vec<&str> = allowed.iter().map(|s| s.as_str()).collect();
        let answer = Self::map_prompt(
            Select::new("Current branch is not allowed. Select target branch:", opts).prompt(),
        )?;
        Ok(answer)
    }
}

pub enum ConfirmAction {
    Yes,
    No,
    Retry,
}

pub fn select_version_step() -> PipelineStep {
    PipelineStep::new("select version", false, |ctx| {
        Box::pin(async move {
            let version = loop {
                let current = ctx
                    .tags
                    .iter()
                    .find(|t| Interactive::parse_version_tag(t).is_some());
                let v = if let Some(latest) = current {
                    Interactive::version_menu(&ctx.tags, latest)
                } else {
                    Interactive::input_version(&ctx.tags, "", true)
                }?;
                match Interactive::confirm_version(&v)? {
                    ConfirmAction::Yes => break v,
                    ConfirmAction::No => {
                        ctx.aborted = true;
                        return Ok(Pipeline::no_rollback());
                    }
                    ConfirmAction::Retry => {}
                }
            };
            ctx.release_version = Some(version);
            Ok(Pipeline::no_rollback())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_tag_accepts_prefixes_and_plain() {
        assert_eq!(
            Interactive::parse_version_tag("v1.2.3").unwrap().to_string(),
            "1.2.3"
        );
        assert_eq!(
            Interactive::parse_version_tag("V2.0.0").unwrap().to_string(),
            "2.0.0"
        );
        assert_eq!(
            Interactive::parse_version_tag("0.1.0").unwrap().to_string(),
            "0.1.0"
        );
        assert_eq!(
            Interactive::parse_version_tag(" 1.0.0 ").unwrap().to_string(),
            "1.0.0"
        );
    }

    #[test]
    fn parse_version_tag_rejects_invalid() {
        assert!(Interactive::parse_version_tag("not-a-version").is_none());
        assert!(Interactive::parse_version_tag("1.2").is_none());
        assert!(Interactive::parse_version_tag("").is_none());
    }
}
