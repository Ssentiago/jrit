use crate::config::models::Config;
use anyhow::Context;
use anyhow::bail;
use std::path::PathBuf;

pub fn load(path: PathBuf) -> anyhow::Result<Config> {
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("jrit.toml not found at {}", path.display()))?;

    let config: Config = toml::from_str(&raw).map_err(|e| {
        eprintln!("warning: jrit.toml structure mismatch: {e}");
        anyhow::anyhow!(e)
    })?;

    validate_config(&config)?;
    Ok(config)
}

fn validate_config(config: &Config) -> anyhow::Result<()> {
    let p = &config.project;
    let mut errors: Vec<String> = Vec::new();

    if p.name.is_empty() {
        errors.push("project.name is empty".into());
    }
    if p.repo.is_empty() {
        errors.push("project.repo is empty".into());
    } else if !p.repo.contains('/') {
        errors.push(format!(
            "project.repo should be in format 'owner/repo' (got: '{}')",
            p.repo
        ));
    }
    if !matches!(
        p.changelog_type.as_str(),
        "conventional" | "raw" | "manual" | "none"
    ) {
        errors.push(format!(
            "unknown changelog_type '{}', expected: conventional, manual, raw, none",
            p.changelog_type
        ));
    }
    if p.changelog_type != "none" && p.changelog.is_none() {
        errors.push("project.changelog is required when changelog_type is not 'none'".into());
    }
    if !matches!(p.release_mode.as_str(), "local" | "ci") {
        errors.push("project.release_mode must be 'local' or 'ci'".into());
    }
    if p.branches.is_empty() {
        errors.push("project.branches is empty".into());
    }
    if config.components.is_empty() {
        errors.push("no [[components]] defined".into());
    }

    let is_ci_mode = p.release_mode.as_str() == "ci";

    for (i, c) in config.components.iter().enumerate() {
        let label = format!("components[{i}]");
        if c.name.is_empty() {
            errors.push(format!("{label}.name is empty"));
        }
        if c.path.is_empty() {
            errors.push(format!("{label}.path is empty"));
        }
        if c.build.is_empty() && !is_ci_mode {
            errors.push(format!("{label}.build is empty"));
        }
        if c.artifact.is_empty() && !is_ci_mode {
            errors.push(format!("{label}.artifact is empty"));
        }
        if c.version_files.is_empty() {
            errors.push(format!("{label}.version_files is empty"));
        }

        for (j, vf) in c.version_files.iter().enumerate() {
            let vf_label = format!("{label}.version_files[{j}]");
            if vf.file.is_empty() {
                errors.push(format!("{vf_label}.file is empty"));
                continue;
            }
            if let Err(e) = vf.resolved_path() {
                errors.push(format!("{vf_label}: {e}"));
            }
        }
    }

    if !errors.is_empty() {
        bail!("config validation failed:\n{}", errors.join("\n"));
    }
    Ok(())
}
