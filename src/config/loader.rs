use crate::config::models::Config;
use anyhow::Context;
use std::path::PathBuf;

pub fn load(path: PathBuf) -> anyhow::Result<Config> {
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("jrit.toml not found at {}", path.display()))?;

    let config: Config = toml::from_str(&raw).map_err(|e| {
        eprintln!("warning: jrit.toml structure mismatch: {e}");
        anyhow::anyhow!(e)
    })?;

    warn_missing(&config);
    Ok(config)
}

fn warn_missing(config: &Config) {
    let p = &config.project;

    if p.name.is_empty() {
        eprintln!("warning: project.name is empty");
    }
    if p.repo.is_empty() {
        eprintln!("warning: project.repo is empty");
    } else if !p.repo.contains('/') {
        eprintln!(
            "warning: project.repo should be in format 'owner/repo' (got: '{}')",
            p.repo
        );
    }
    if !matches!(p.changelog_type.as_str(), "conventional" | "raw" | "none") {
        eprintln!(
            "warning: unknown changelog_type '{}', expected: conventional, raw, none",
            p.changelog_type
        );
    }
    if p.changelog_type != "none" && p.changelog.is_none() {
        eprintln!("warning: project.changelog is required when changelog_type is not 'none'");
    }
    if !matches!(p.release_mode.as_str(), "local" | "ci") {
        eprintln!("warning: project.release_mode must be 'local' or 'ci'");
    }
    if p.branches.is_empty() {
        eprintln!("warning: project.branches is empty");
    }
    if config.components.is_empty() {
        eprintln!("warning: no [[components]] defined");
    }

    for (i, c) in config.components.iter().enumerate() {
        let label = format!("components[{i}]");
        if c.name.is_empty() {
            eprintln!("warning: {label}.name is empty");
        }
        if c.path.is_empty() {
            eprintln!("warning: {label}.path is empty");
        }
        if c.build.is_empty() {
            eprintln!("warning: {label}.build is empty");
        }
        if c.artifact.is_empty() {
            eprintln!("warning: {label}.artifact is empty");
        }
        if c.version_files.is_empty() {
            eprintln!("warning: {label}.version_files is empty");
        }

        for (j, vf) in c.version_files.iter().enumerate() {
            let vf_label = format!("{label}.version_files[{j}]");
            if vf.file.is_empty() {
                eprintln!("warning: {vf_label}.file is empty");
                continue;
            }
            if let Err(e) = vf.resolved_path() {
                eprintln!("warning: {vf_label}: {e}");
            }
        }
    }
}
