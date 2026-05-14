use anyhow::{Result, bail};
use inquire::{InquireError, Select, Text};
use semver::Version;

pub struct Interactive;

impl Interactive {
    pub fn version_menu(tags: &[String], current: &str) -> Result<String> {
        let parsed = Version::parse(current)?;
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
            let answer = Select::new(
                &format!("Current version: {current}. Select action:"),
                options.clone(),
            )
            .prompt()?;

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
            let _ = Text::new("Press Enter to continue...").prompt();
        }
    }

    pub fn input_version(tags: &[String], current: &str, is_first: bool) -> Result<String> {
        let current_parsed = if is_first {
            None
        } else {
            Some(Version::parse(current)?)
        };
        let tags = tags.to_vec();

        let version = match Text::new("Enter new version (semver) or leave empty to exit:")
            .with_validator(move |v: &str| {
                if v.trim().is_empty() {
                    return Ok(inquire::validator::Validation::Valid);
                }
                let Ok(parsed) = Version::parse(v) else {
                    return Ok(inquire::validator::Validation::Invalid(
                        "Invalid semver format (e.g. 1.2.3)".into(),
                    ));
                };
                if tags.contains(&v.to_string()) {
                    return Ok(inquire::validator::Validation::Invalid(
                        "Version already exists.".into(),
                    ));
                }
                if let Some(ref cur) = current_parsed {
                    if &parsed < cur {
                        return Ok(inquire::validator::Validation::Invalid(
                            format!("Must be greater than current version {cur}").into(),
                        ));
                    }
                }
                Ok(inquire::validator::Validation::Valid)
            })
            .prompt()
        {
            Ok(v) => v,
            Err(InquireError::OperationInterrupted) => {
                println!("\nSee you later!");
                std::process::exit(0);
            }
            Err(e) => return Err(e.into()),
        };

        if version.trim().is_empty() {
            bail!("exit");
        }
        Ok(version.trim().to_string())
    }

    pub fn confirm_version(version: &str) -> Result<ConfirmAction> {
        let options = vec!["Yes", "No", "Retry"];
        let answer =
            Select::new(&format!("Selected version: {version}. Continue?"), options).prompt()?;

        match answer {
            "Yes" => Ok(ConfirmAction::Yes),
            "No" => Ok(ConfirmAction::No),
            "Retry" => Ok(ConfirmAction::Retry),
            _ => bail!("unexpected answer"),
        }
    }

    pub fn select_branch(allowed: &[String]) -> Result<&str> {
        let opts: Vec<&str> = allowed.iter().map(|s| s.as_str()).collect();
        let answer =
            Select::new("Current branch is not allowed. Select target branch:", opts).prompt()?;
        Ok(answer)
    }
}

pub enum ConfirmAction {
    Yes,
    No,
    Retry,
}
