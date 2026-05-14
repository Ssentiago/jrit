use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Project {
    pub name: String,
    pub repo: String,
    pub branches: Vec<String>,
    #[serde(default = "default_changelog_type")]
    pub changelog_type: String,
    pub changelog: Option<String>,
    #[serde(default = "default_release_mode")]
    pub release_mode: String,
}

fn default_changelog_type() -> String {
    "none".to_string()
}

fn default_release_mode() -> String {
    "local".to_string()
}

impl Project {
    pub fn owner(&self) -> String {
        self.repo.split('/').next().unwrap_or("").to_string()
    }

    pub fn repo_name(&self) -> String {
        self.repo.split('/').nth(1).unwrap_or("").to_string()
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct VersionFile {
    pub file: String,
    #[serde(default)]
    pub path: Option<Vec<String>>,
}

impl VersionFile {
    pub fn resolved_path(&self) -> anyhow::Result<Vec<String>> {
        if let Some(ref p) = self.path {
            if !p.is_empty() {
                return Ok(p.clone());
            }
        }

        let filename = PathBuf::from(&self.file)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        match filename.as_str() {
            "Cargo.toml" => Ok(vec!["package".into(), "version".into()]),
            "package.json" | "manifest.json" => Ok(vec!["version".into()]),
            other => bail!(
                "cannot infer version path for '{}': please specify path explicitly",
                other
            ),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Component {
    pub name: String,
    pub path: String,
    pub build: String,
    pub artifact: String,
    pub version_files: Vec<VersionFile>,
    #[serde(default = "default_zip")]
    pub zip: bool,
}

fn default_zip() -> bool {
    true
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Config {
    pub project: Project,
    pub components: Vec<Component>,
}
