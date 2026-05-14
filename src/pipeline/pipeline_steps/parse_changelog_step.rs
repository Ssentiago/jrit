use anyhow::bail;
use fancy_regex::Regex;
use fancy_regex::escape;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Changelog {
    pub version: String,
    pub body: String,
}

impl Changelog {
    pub fn parse(version: &str, path: PathBuf) -> anyhow::Result<Changelog> {
        if !path.exists() {
            bail!("Changelog at path {} not found", path.display());
        }

        let content = std::fs::read_to_string(path)?;

        let escaped_version = escape(&version);
        let pattern = Regex::new(&format!(
            r"(?ms)^## \[{version}\](.*?)(?=^## |\z)",
            version = escaped_version
        ))?;

        let regex_match = pattern.captures(&content)?;

        let body = regex_match
            .and_then(|m| m.get(1))
            .map(|m| m.as_str().trim())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "✗ changelog section for {} not found in CHANGELOG.md
  add the following to your changelog and re-run:
  ## [{}]
  ### Features
  - ...",
                    version,
                    version
                )
            })?;

        anyhow::Ok(Self {
            version: version.to_string(),
            body: body.to_string(),
        })
    }
}
