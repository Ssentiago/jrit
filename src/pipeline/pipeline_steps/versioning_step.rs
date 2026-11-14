use crate::pipeline::{Pipeline, PipelineStep, Rollback};
use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, TableLike, Value as TomlValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionFormat {
    Toml,
    Json,
}

impl VersionFormat {
    pub fn from_path(path: &Path) -> Result<Self> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("toml") => Ok(Self::Toml),
            Some("json") => Ok(Self::Json),
            Some(ext) => anyhow::bail!("unknown version file type: {ext}"),
            None => anyhow::bail!("no file extension: {}", path.display()),
        }
    }
}

pub struct VersionBumper {
    pub path: PathBuf,
    pub fields: Vec<String>,
    format: VersionFormat,
    original_content: Option<String>,
}

/// The version change a bumper would make to a file.
pub struct VersionPreview {
    pub old: String,
    pub new: String,
}

impl VersionBumper {
    pub fn from_file(path: PathBuf, fields: Vec<String>) -> Result<Self> {
        let format = VersionFormat::from_path(&path)?;
        Ok(Self {
            path,
            fields,
            format,
            original_content: None,
        })
    }

    fn read_content(&self) -> Result<String> {
        std::fs::read_to_string(&self.path)
            .with_context(|| format!("failed to read {}", self.path.display()))
    }

    fn read_original(&mut self) -> Result<String> {
        let content = self.read_content()?;
        self.original_content = Some(content.clone());
        Ok(content)
    }

    pub fn execute(&mut self, new_version: &str) -> Result<()> {
        let content = self.read_original()?;
        match self.format {
            VersionFormat::Toml => self.bump_toml(&content, new_version),
            VersionFormat::Json => self.bump_json(&content, new_version),
        }
    }

    /// Computes the version change for the file without writing anything.
    pub fn preview(&self, new_version: &str) -> Result<VersionPreview> {
        let content = self.read_content()?;
        let old = match self.format {
            VersionFormat::Toml => {
                let mut doc: DocumentMut = content
                    .parse()
                    .with_context(|| format!("failed to parse TOML: {}", self.path.display()))?;
                let value = self.find_toml_value(&mut doc)?;
                value
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| value.to_string().trim().to_string())
            }
            VersionFormat::Json => {
                let mut json: JsonValue = serde_json::from_str(&content)
                    .with_context(|| format!("failed to parse JSON: {}", self.path.display()))?;
                let value = self.find_json_value(&mut json)?;
                value
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| value.to_string())
            }
        };
        Ok(VersionPreview {
            old,
            new: new_version.to_string(),
        })
    }

    /// Navigates to the configured version value, shared by preview and write
    /// so the two can never look at different fields.
    fn find_toml_value<'a>(&self, doc: &'a mut DocumentMut) -> Result<&'a mut TomlValue> {
        let (last_field, parents) = self.fields.split_last().with_context(|| {
            format!(
                "no version path configured for {}",
                self.path.display()
            )
        })?;

        let mut cur: &mut dyn TableLike = &mut **doc;
        for field in parents {
            let item = cur.get_mut(field.as_str()).with_context(|| {
                format!("field '{}' not found in {}", field, self.path.display())
            })?;
            cur = item.as_table_like_mut().with_context(|| {
                format!("field '{}' in {} is not a table", field, self.path.display())
            })?;
        }

        let target = cur.get_mut(last_field.as_str()).with_context(|| {
            format!(
                "field '{}' not found in {}",
                last_field,
                self.path.display()
            )
        })?;
        target.as_value_mut().with_context(|| {
            format!(
                "field '{}' in {} is not a value",
                last_field,
                self.path.display()
            )
        })
    }

    fn find_json_value<'a>(&self, json: &'a mut JsonValue) -> Result<&'a mut JsonValue> {
        let mut cur = json;
        for field in &self.fields {
            cur = cur.get_mut(field.as_str()).with_context(|| {
                format!("field '{}' not found in {}", field, self.path.display())
            })?;
        }
        Ok(cur)
    }

    fn bump_toml(&self, content: &str, new_version: &str) -> Result<()> {
        let mut doc: DocumentMut = content
            .parse()
            .with_context(|| format!("failed to parse TOML: {}", self.path.display()))?;

        let value = self.find_toml_value(&mut doc)?;
        let old_decor = value.decor().clone();
        let mut new_value = TomlValue::from(new_version);
        *new_value.decor_mut() = old_decor;
        *value = new_value;

        std::fs::write(&self.path, doc.to_string())
            .with_context(|| format!("failed to write {}", self.path.display()))
    }

    fn bump_json(&self, content: &str, new_version: &str) -> Result<()> {
        let mut json: JsonValue = serde_json::from_str(content)
            .with_context(|| format!("failed to parse JSON: {}", self.path.display()))?;

        let value = self.find_json_value(&mut json)?;
        *value = JsonValue::String(new_version.to_string());

        let updated = serde_json::to_string_pretty(&json)
            .with_context(|| format!("failed to serialize JSON: {}", self.path.display()))?;

        std::fs::write(&self.path, updated)
            .with_context(|| format!("failed to write {}", self.path.display()))
    }

    pub fn rollback(&mut self) -> Result<()> {
        if let Some(original) = &self.original_content {
            std::fs::write(&self.path, original)
                .with_context(|| format!("rollback failed for {}", self.path.display()))?;
        }
        Ok(())
    }

    pub fn into_step(mut self, new_version: String) -> PipelineStep {
        PipelineStep::new(format!("bump {}", self.path.display()), false, move |_ctx| {
            Box::pin(async move {
                self.execute(&new_version)?;
                let rollback: Rollback = Box::new(move || self.rollback());
                Ok(rollback)
            })
        })
    }
}

pub fn init_bumpers_step() -> PipelineStep {
    PipelineStep::new("init bumpers", true, |ctx| {
        Box::pin(async move {
            let config = ctx.config.as_ref().context("config not loaded")?;
            let version = ctx
                .release_version
                .as_ref()
                .context("release version not selected")?
                .clone();
            let root = ctx
                .root
                .as_ref()
                .context("project root not determined")?
                .clone();

            let mut bumped_files: Vec<PathBuf> = Vec::new();
            let mut steps: Vec<PipelineStep> = Vec::new();
            for component in &config.components {
                for vf in &component.version_files {
                    let resolved = vf.resolved_path()?;
                    let full_path = root.join(&component.path).join(&vf.file);
                    bumped_files.push(full_path.clone());
                    let bumper = VersionBumper::from_file(full_path, resolved)?;
                    steps.push(bumper.into_step(version.clone()));
                }
            }

            ctx.bumped_files = bumped_files;
            for step in steps {
                ctx.pending_steps.push(step);
            }
            Ok(Pipeline::no_rollback())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toml_bumper(file: &Path) -> VersionBumper {
        VersionBumper::from_file(file.to_path_buf(), vec!["package".into(), "version".into()])
            .unwrap()
    }

    #[test]
    fn toml_bump_preserves_comments_and_format() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Cargo.toml");
        let original = "# top comment\n[package]\nname = \"demo\" # trailing\nversion = \"0.1.0\"\n\n[dependencies]\nanyhow = \"1\" # dep comment\n";
        std::fs::write(&file, original).unwrap();

        let mut bumper = toml_bumper(&file);
        bumper.execute("0.2.0").unwrap();

        let updated = std::fs::read_to_string(&file).unwrap();
        assert!(updated.contains("# top comment"));
        assert!(updated.contains("name = \"demo\" # trailing"));
        assert!(updated.contains("version = \"0.2.0\""));
        assert!(updated.contains("anyhow = \"1\" # dep comment"));
        assert!(!updated.contains("0.1.0"));

        bumper.rollback().unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
    }

    #[test]
    fn toml_bump_missing_field_errors() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Cargo.toml");
        std::fs::write(&file, "[package]\nname = \"demo\"\n").unwrap();

        let mut bumper = toml_bumper(&file);
        assert!(bumper.execute("0.2.0").is_err());
    }

    #[test]
    fn toml_preview_reports_old_value_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Cargo.toml");
        let original = "# top comment\n[package]\nname = \"demo\" # trailing\nversion = \"0.1.0\"\n";
        std::fs::write(&file, original).unwrap();

        let bumper = toml_bumper(&file);
        let preview = bumper.preview("0.2.0").unwrap();

        assert_eq!(preview.old, "0.1.0");
        assert_eq!(preview.new, "0.2.0");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
    }

    #[test]
    fn toml_preview_missing_field_errors() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Cargo.toml");
        std::fs::write(&file, "[package]\nname = \"demo\"\n").unwrap();

        let bumper = toml_bumper(&file);
        assert!(bumper.preview("0.2.0").is_err());
    }

    #[test]
    fn json_bump_sets_version_and_rolls_back() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("package.json");
        let original = "{\n  \"name\": \"demo\",\n  \"version\": \"0.1.0\"\n}";
        std::fs::write(&file, original).unwrap();

        let mut bumper = VersionBumper::from_file(file.clone(), vec!["version".into()]).unwrap();
        bumper.execute("0.2.0").unwrap();

        let updated = std::fs::read_to_string(&file).unwrap();
        assert!(updated.contains("\"0.2.0\""));
        assert!(!updated.contains("\"0.1.0\""));

        bumper.rollback().unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
    }

    #[test]
    fn json_bump_missing_field_errors() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("package.json");
        std::fs::write(&file, "{\n  \"name\": \"demo\"\n}").unwrap();

        let mut bumper = VersionBumper::from_file(file, vec!["version".into()]).unwrap();
        assert!(bumper.execute("0.2.0").is_err());
    }

    #[test]
    fn json_preview_reports_old_value_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("package.json");
        let original = "{\n  \"name\": \"demo\",\n  \"version\": \"0.1.0\"\n}";
        std::fs::write(&file, original).unwrap();

        let bumper = VersionBumper::from_file(file.clone(), vec!["version".into()]).unwrap();
        let preview = bumper.preview("0.2.0").unwrap();

        assert_eq!(preview.old, "0.1.0");
        assert_eq!(preview.new, "0.2.0");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
    }

    #[test]
    fn unknown_extension_errors() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("version.txt");
        std::fs::write(&file, "1").unwrap();
        assert!(VersionBumper::from_file(file, vec!["version".into()]).is_err());
    }
}
