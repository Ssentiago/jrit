use anyhow::{Result, bail};
use octocrab;
use reqwest::Response;
use self_replace;
use std::io::Write;

pub async fn main() -> Result<()> {
    let current_version = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;

    println!("checking for updates...");

    let octo_client = octocrab::instance();

    let latest_release = octo_client
        .repos("Ssentiago", "jrit")
        .releases()
        .get_latest()
        .await?;

    let latest_release_tag = semver::Version::parse(&latest_release.tag_name)?;

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
        let bytes = response.bytes().await?;
        let mut tmp = tempfile::NamedTempFile::new()?;
        tmp.write_all(&bytes)?;
        self_replace::self_replace(tmp.path())?;

        println!("updated to {latest_release_tag}, please restart jrit");
    } else {
        println!("already up to date ({current_version})");
    }

    Ok(())
}
