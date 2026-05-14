use anyhow::{Context, Result};
use bytes::Bytes;
use octocrab::Octocrab;
use std::path::{Path, PathBuf};

pub struct ReleaseParams {
    pub repo_owner: String,
    pub repo_name: String,
    pub tag: String,
    pub name: String,
    pub body: String,
    pub artifacts: Vec<PathBuf>,
    pub draft: bool,
    pub prerelease: bool,
}

pub struct ReleaseResult {
    pub url: String,
    pub id: u64,
}

pub async fn get_token() -> Result<String> {
    // first trying gh CLI
    let gh = std::process::Command::new("gh")
        .args(["auth", "token"])
        .output();

    if let Ok(out) = gh {
        if out.status.success() {
            let token = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !token.is_empty() {
                return Ok(token);
            }
        }
    }

    // fallback: ~/.config/jrit/config.toml
    let config_path = dirs::config_dir()
        .context("Cannot resolve config dir")?
        .join("jrit/config.toml");

    let raw = std::fs::read_to_string(&config_path)
        .context("gh CLI not available and ~/.config/jrit/config.toml not found")?;

    let config: toml::Value = toml::from_str(&raw)?;
    config["github"]["token"]
        .as_str()
        .map(|s| s.to_string())
        .context("github.token not found in jrit config")
}

pub fn build_release_body(
    changelog: &str,
    prev_tag: Option<&str>,
    new_tag: &str,
    repo: &str,
) -> String {
    match prev_tag {
        Some(prev) => {
            let url = format!("https://github.com/{repo}/compare/{prev}...{new_tag}");
            format!("{changelog}\n\nFull changelog: {url}")
        }
        None => changelog.to_string(),
    }
}

fn sha256_of(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).with_context(|| format!("Cannot read {:?}", path))?;
    let hash = Sha256::digest(&bytes);
    Ok(hex::encode(hash))
}

pub async fn create_release(params: ReleaseParams) -> Result<ReleaseResult> {
    let token = get_token().await?;
    let octocrab = Octocrab::builder().personal_token(token).build()?;

    let release = octocrab
        .repos(&params.repo_owner, &params.repo_name)
        .releases()
        .create(&params.tag)
        .name(&format!("Release {}", params.tag))
        .body(&params.body)
        .draft(params.draft)
        .prerelease(params.prerelease)
        .send()
        .await
        .context("Failed to create GitHub release")?;

    for artifact in &params.artifacts {
        let file_name = artifact
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let bytes = std::fs::read(artifact)
            .with_context(|| format!("Cannot read artifact {:?}", artifact))?;

        octocrab
            .repos(&params.repo_owner, &params.repo_name)
            .releases()
            .upload_asset(release.id.0, &file_name, Bytes::from(bytes))
            .send()
            .await
            .with_context(|| format!("Failed to upload {file_name}"))?;
    }

    Ok(ReleaseResult {
        url: release.html_url.to_string(),
        id: release.id.0,
    })
}
