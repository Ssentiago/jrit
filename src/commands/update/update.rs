use anyhow::{Result, bail};
use octocrab;
use reqwest::Response;
use self_replace;
use std::io::Write;

fn parse_version(tag: &str) -> Result<semver::Version> {
    let tag = tag.strip_prefix('v').unwrap_or(tag);
    Ok(semver::Version::parse(tag)?)
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

fn find_checksum(sums: &str, name: &str) -> Option<String> {
    sums.lines()
        .filter_map(|line| line.split_once("  "))
        .find(|(_, file)| file.trim() == name)
        .map(|(hash, _)| hash.trim().to_string())
}

pub async fn main() -> Result<()> {
    let current_version = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;

    println!("checking for updates...");

    let octo_client = octocrab::instance();

    let latest_release = octo_client
        .repos("Ssentiago", "jrit")
        .releases()
        .get_latest()
        .await?;

    let latest_release_tag = parse_version(&latest_release.tag_name)?;

    if latest_release_tag > current_version {
        println!("new version available: {current_version} → {latest_release_tag}");

        let asset = if cfg!(target_os = "macos") {
            latest_release
                .assets
                .iter()
                .find(|a| a.name == "jrit-macos")
        } else if cfg!(target_os = "linux") {
            latest_release
                .assets
                .iter()
                .find(|a| a.name == "jrit-linux")
        } else if cfg!(target_os = "windows") {
            latest_release
                .assets
                .iter()
                .find(|a| a.name == "jrit-windows.exe")
        } else {
            None
        };

        let asset = asset.ok_or_else(|| anyhow::anyhow!("no asset found for current platform"))?;

        println!("downloading...");

        let response: Response = reqwest::get(asset.browser_download_url.as_str()).await?;
        let response = response.error_for_status()?;
        let bytes = response.bytes().await?;
        if bytes.is_empty() {
            bail!("downloaded release asset is empty, binary left untouched");
        }

        match latest_release
            .assets
            .iter()
            .find(|a| a.name == "sha256sums.txt")
        {
            Some(sums) => {
                let response = reqwest::get(sums.browser_download_url.as_str())
                    .await?
                    .error_for_status()?;
                let sums_text = response.text().await?;
                let expected = find_checksum(&sums_text, &asset.name).ok_or_else(|| {
                    anyhow::anyhow!("no checksum entry for {} in sha256sums.txt", asset.name)
                })?;
                let actual = sha256_hex(&bytes);
                if actual != expected {
                    bail!(
                        "checksum mismatch for {}: expected {}, got {}, binary left untouched",
                        asset.name,
                        expected,
                        actual
                    );
                }
                println!("checksum verified");
            }
            None => {
                println!("warning: sha256sums.txt missing in release, skipping checksum verification");
            }
        }

        let mut tmp = tempfile::NamedTempFile::new()?;
        tmp.write_all(&bytes)?;
        self_replace::self_replace(tmp.path())?;

        println!("updated to {latest_release_tag}, please restart jrit");
    } else {
        println!("already up to date ({current_version})");
    }

    Ok(())
}
