use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ChangelogType {
    #[default]
    None,
    Conventional,
    Raw,
    Manual,
}

impl FromStr for ChangelogType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "conventional" => Ok(Self::Conventional),
            "raw" => Ok(Self::Raw),
            "manual" => Ok(Self::Manual),
            other => bail!(
                "unknown changelog_type '{other}', expected: conventional, manual, raw, none"
            ),
        }
    }
}

impl ChangelogType {
    pub fn is_generated(self) -> bool {
        matches!(self, Self::Conventional | Self::Raw)
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseMode {
    #[default]
    Local,
    Ci,
}

impl FromStr for ReleaseMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "ci" => Ok(Self::Ci),
            other => bail!("unknown release_mode '{other}', expected: local, ci"),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Project {
    pub name: String,
    pub repo: String,
    pub branches: Vec<String>,
    #[serde(default)]
    pub changelog_type: ChangelogType,
    pub changelog: Option<String>,
    #[serde(default)]
    pub release_mode: ReleaseMode,
    #[serde(default)]
    pub sha256sums: bool,
    #[serde(default = "default_release_name")]
    pub release_name: String,
}

pub fn default_release_name() -> String {
    "Release {{ version }}".to_string()
}

/// Renders a release name template, substituting the `{{ version }}` variable.
/// Unknown variables and unclosed placeholders are rejected.
pub fn render_release_name(template: &str, version: &str) -> anyhow::Result<String> {
    if template.trim().is_empty() {
        bail!("release_name template is empty");
    }

    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        let Some(close) = after.find("}}") else {
            bail!("release_name template has an unclosed '{{' placeholder");
        };
        match after[..close].trim() {
            "version" => out.push_str(version),
            other => bail!("unknown release_name variable '{{{{ {other} }}}}', expected: version"),
        }
        rest = &after[close + 2..];
    }
    out.push_str(rest);
    Ok(out)
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
        if let Some(ref p) = self.path
            && !p.is_empty()
        {
            return Ok(p.clone());
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
    #[serde(default)]
    pub build: String,
    #[serde(default)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changelog_type_parses_and_rejects() {
        assert_eq!(
            "none".parse::<ChangelogType>().unwrap(),
            ChangelogType::None
        );
        assert_eq!(
            "conventional".parse::<ChangelogType>().unwrap(),
            ChangelogType::Conventional
        );
        assert_eq!("raw".parse::<ChangelogType>().unwrap(), ChangelogType::Raw);
        assert_eq!(
            "manual".parse::<ChangelogType>().unwrap(),
            ChangelogType::Manual
        );
        assert_eq!(
            "Conventional".parse::<ChangelogType>().unwrap(),
            ChangelogType::Conventional
        );
        assert!("bogus".parse::<ChangelogType>().is_err());
    }

    #[test]
    fn release_mode_parses_and_rejects() {
        assert_eq!(
            "local".parse::<ReleaseMode>().unwrap(),
            ReleaseMode::Local
        );
        assert_eq!("ci".parse::<ReleaseMode>().unwrap(), ReleaseMode::Ci);
        assert!("netlify".parse::<ReleaseMode>().is_err());
    }

    #[test]
    fn enums_serialize_to_lowercase_strings() {
        assert_eq!(
            serde_json::to_string(&ChangelogType::None).unwrap(),
            "\"none\""
        );
        assert_eq!(serde_json::to_string(&ReleaseMode::Ci).unwrap(), "\"ci\"");
        assert_eq!(
            serde_json::from_str::<ChangelogType>("\"raw\"").unwrap(),
            ChangelogType::Raw
        );
    }

    #[test]
    fn resolved_path_infers_and_respects_explicit() {
        let cargo = VersionFile {
            file: "Cargo.toml".into(),
            path: None,
        };
        assert_eq!(cargo.resolved_path().unwrap(), vec!["package", "version"]);

        let pkg = VersionFile {
            file: "package.json".into(),
            path: None,
        };
        assert_eq!(pkg.resolved_path().unwrap(), vec!["version"]);

        let explicit = VersionFile {
            file: "weird.toml".into(),
            path: Some(vec!["a".into(), "b".into()]),
        };
        assert_eq!(explicit.resolved_path().unwrap(), vec!["a", "b"]);

        let unknown = VersionFile {
            file: "thing.toml".into(),
            path: None,
        };
        assert!(unknown.resolved_path().is_err());
    }

    #[test]
    fn sha256sums_is_opt_in() {
        let head = "[project]\nname = \"demo\"\nrepo = \"owner/demo\"\nbranches = [\"main\"]\n";
        let tail = "[[components]]\nname = \"main\"\npath = \".\"\nbuild = \"cargo build\"\nartifact = \"./a\"\nversion_files = [{ file = \"Cargo.toml\" }]\n";

        let off: Config = toml::from_str(&format!("{head}{tail}")).unwrap();
        assert!(!off.project.sha256sums);

        let on: Config = toml::from_str(&format!("{head}sha256sums = true\n{tail}")).unwrap();
        assert!(on.project.sha256sums);
    }

    #[test]
    fn release_name_defaults_to_template() {
        let head = "[project]\nname = \"demo\"\nrepo = \"owner/demo\"\nbranches = [\"main\"]\n";
        let tail = "[[components]]\nname = \"main\"\npath = \".\"\nbuild = \"cargo build\"\nartifact = \"./a\"\nversion_files = [{ file = \"Cargo.toml\" }]\n";

        let config: Config = toml::from_str(&format!("{head}{tail}")).unwrap();
        assert_eq!(config.project.release_name, "Release {{ version }}");
    }

    #[test]
    fn render_release_name_substitutes_version() {
        assert_eq!(
            render_release_name("Release {{ version }}", "1.2.3").unwrap(),
            "Release 1.2.3"
        );
        assert_eq!(
            render_release_name("Release{{version}}", "1.2.3").unwrap(),
            "Release1.2.3"
        );
        assert_eq!(
            render_release_name("{{ version }} is out", "1.2.3").unwrap(),
            "1.2.3 is out"
        );
        assert_eq!(render_release_name("static name", "1.2.3").unwrap(), "static name");
    }

    #[test]
    fn render_release_name_rejects_bad_templates() {
        assert!(render_release_name("", "1.2.3").is_err());
        assert!(render_release_name("   ", "1.2.3").is_err());
        assert!(render_release_name("Release {{ version", "1.2.3").is_err());
        assert!(render_release_name("Release {{ nope }}", "1.2.3").is_err());
        assert!(render_release_name("Release {{ }}", "1.2.3").is_err());
    }

    #[test]
    fn release_name_round_trips_through_toml() {
        let config: Config = toml::from_str(
            "[project]\nname = \"demo\"\nrepo = \"owner/demo\"\nbranches = [\"main\"]\nrelease_name = \"demo {{ version }}\"\n[[components]]\nname = \"main\"\npath = \".\"\nversion_files = [{ file = \"Cargo.toml\" }]\n",
        )
        .unwrap();
        assert_eq!(config.project.release_name, "demo {{ version }}");
    }
}
