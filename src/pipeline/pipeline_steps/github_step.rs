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

    for var in ["GITHUB_TOKEN", "GH_TOKEN"] {
        if let Ok(token) = std::env::var(var) {
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

fn release_title(name: &str, tag: &str) -> String {
    if name.is_empty() {
        format!("Release {tag}")
    } else {
        format!("{name} {tag}")
    }
}

fn build_checksum_file(artifacts: &[PathBuf]) -> Result<String> {
    let mut out = String::new();
    for artifact in artifacts {
        let name = artifact
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        out.push_str(&format!("{}  {}\n", sha256_of(artifact)?, name));
    }
    Ok(out)
}

async fn upload_assets(
    octocrab: &Octocrab,
    params: &ReleaseParams,
    release_id: u64,
) -> Result<()> {
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
            .upload_asset(release_id, &file_name, Bytes::from(bytes))
            .send()
            .await
            .with_context(|| format!("Failed to upload {file_name}"))?;
    }

    let checksums = build_checksum_file(&params.artifacts)?;
    octocrab
        .repos(&params.repo_owner, &params.repo_name)
        .releases()
        .upload_asset(release_id, "sha256sums.txt", Bytes::from(checksums))
        .send()
        .await
        .context("Failed to upload sha256sums.txt")?;

    Ok(())
}

pub async fn create_release(params: ReleaseParams) -> Result<ReleaseResult> {
    let token = get_token().await?;
    let octocrab = Octocrab::builder().personal_token(token).build()?;
    let repo = octocrab.repos(&params.repo_owner, &params.repo_name);
    let releases = repo.releases();

    let release = releases
        .create(&params.tag)
        .name(&release_title(&params.name, &params.tag))
        .body(&params.body)
        .draft(params.draft)
        .prerelease(params.prerelease)
        .send()
        .await
        .context("Failed to create GitHub release")?;

    if let Err(err) = upload_assets(&octocrab, &params, release.id.0).await {
        // The pipeline rolls the tag back on failure, so a release left behind
        // would stay orphaned; drop it before surfacing the upload error.
        return match releases.delete(release.id.0).await {
            Ok(()) => Err(err.context(format!(
                "release {} was deleted after the failed upload",
                release.id.0
            ))),
            Err(delete_err) => Err(err.context(format!(
                "release {} is still published, delete it manually: {delete_err:#}",
                release.id.0
            ))),
        };
    }

    Ok(ReleaseResult {
        url: release.html_url.to_string(),
        id: release.id.0,
    })
}
