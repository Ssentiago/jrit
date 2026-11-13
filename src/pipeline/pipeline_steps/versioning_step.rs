use crate::pipeline::{PipelineStep, Rollback};
use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use std::path::PathBuf;
use toml_edit::{DocumentMut, TableLike, Value as TomlValue};

pub trait VersionBumper {
    fn path(&self) -> &PathBuf;
    fn execute(&mut self, new_version: &str) -> Result<()>;
    fn rollback(&mut self) -> Result<()>;
    fn into_step(self: Box<Self>, new_version: String) -> PipelineStep;
}

pub struct TomlVersionBumper {
    pub relative_path: PathBuf,
    pub fields: Vec<String>,
    original_content: Option<String>,
}

pub struct JsonVersionBumper {
    pub relative_path: PathBuf,
    pub fields: Vec<String>,
    original_content: Option<String>,
}

impl TomlVersionBumper {
    pub fn new(relative_path: PathBuf, fields: Vec<String>) -> Self {
        Self {
            relative_path,
            fields,
            original_content: None,
        }
    }
}

impl JsonVersionBumper {
    pub fn new(relative_path: PathBuf, fields: Vec<String>) -> Self {
        Self {
            relative_path,
            fields,
            original_content: None,
        }
    }
}

impl VersionBumper for TomlVersionBumper {
    fn path(&self) -> &PathBuf {
        &self.relative_path
    }

    fn into_step(mut self: Box<Self>, new_version: String) -> PipelineStep {
        let name = format!("bump {}", self.relative_path.display());
        PipelineStep {
            name,
            run: Box::new(move |_ctx| {
                Box::pin(async move {
                    self.execute(&new_version)?;
                    let rollback: Rollback = Box::new(move || self.rollback());
                    Ok(rollback)
                })
            }),
            silent: false,
        }
    }
    fn execute(&mut self, new_version: &str) -> Result<()> {
        let content = std::fs::read_to_string(&self.relative_path)
            .with_context(|| format!("failed to read {}", self.relative_path.display()))?;

        self.original_content = Some(content.clone());

        let mut doc: DocumentMut = content.parse().with_context(|| {
            format!("failed to parse TOML: {}", self.relative_path.display())
        })?;

        let (last_field, parents) = self.fields.split_last().with_context(|| {
            format!(
                "no version path configured for {}",
                self.relative_path.display()
            )
        })?;

        let mut cur: &mut dyn TableLike = &mut *doc;
        for field in parents {
            let item = cur.get_mut(field.as_str()).with_context(|| {
                format!(
                    "field '{}' not found in {}",
                    field,
                    self.relative_path.display()
                )
            })?;
            cur = item.as_table_like_mut().with_context(|| {
                format!(
                    "field '{}' in {} is not a table",
                    field,
                    self.relative_path.display()
                )
            })?;
        }

        let target = cur.get_mut(last_field.as_str()).with_context(|| {
            format!(
                "field '{}' not found in {}",
                last_field,
                self.relative_path.display()
            )
        })?;

        let old_decor = target.as_value().map(|v| v.decor().clone());
        let mut new_value = TomlValue::from(new_version);
        if let Some(decor) = old_decor {
            *new_value.decor_mut() = decor;
        }
        *target = toml_edit::Item::Value(new_value);

        std::fs::write(&self.relative_path, doc.to_string())
            .with_context(|| format!("failed to write {}", self.relative_path.display()))?;

        Ok(())
    }

    fn rollback(&mut self) -> Result<()> {
        if let Some(original) = &self.original_content {
            std::fs::write(&self.relative_path, original)
                .with_context(|| format!("rollback failed for {}", self.relative_path.display()))?;
        }
        Ok(())
    }
}

impl VersionBumper for JsonVersionBumper {
    fn path(&self) -> &PathBuf {
        &self.relative_path
    }

    fn into_step(mut self: Box<Self>, new_version: String) -> PipelineStep {
        let name = format!("bump {}", self.relative_path.display());
        PipelineStep {
            name,
            run: Box::new(move |_ctx| {
                Box::pin(async move {
                    self.execute(&new_version)?;
                    let rollback: Rollback = Box::new(move || self.rollback());
                    Ok(rollback)
                })
            }),
            silent: false,
        }
    }
    fn execute(&mut self, new_version: &str) -> Result<()> {
        let content = std::fs::read_to_string(&self.relative_path)
            .with_context(|| format!("failed to read {}", self.relative_path.display()))?;

        self.original_content = Some(content.clone());

        let mut json: JsonValue = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse JSON: {}", self.relative_path.display()))?;

        let mut cur = &mut json;
        for field in &self.fields {
            cur = cur.get_mut(field.as_str()).with_context(|| {
                format!(
                    "field '{}' not found in {}",
                    field,
                    self.relative_path.display()
                )
            })?;
        }
        *cur = JsonValue::String(new_version.to_string());

        let updated = serde_json::to_string_pretty(&json).with_context(|| {
            format!("failed to serialize JSON: {}", self.relative_path.display())
        })?;

        std::fs::write(&self.relative_path, updated)
            .with_context(|| format!("failed to write {}", self.relative_path.display()))?;

        Ok(())
    }

    fn rollback(&mut self) -> Result<()> {
        if let Some(original) = &self.original_content {
            std::fs::write(&self.relative_path, original)
                .with_context(|| format!("rollback failed for {}", self.relative_path.display()))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_bump_preserves_comments_and_format() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Cargo.toml");
        let original = "# top comment\n[package]\nname = \"demo\" # trailing\nversion = \"0.1.0\"\n\n[dependencies]\nanyhow = \"1\" # dep comment\n";
        std::fs::write(&file, original).unwrap();

        let mut bumper =
            TomlVersionBumper::new(file.clone(), vec!["package".into(), "version".into()]);
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

        let mut bumper = TomlVersionBumper::new(file, vec!["package".into(), "version".into()]);
        assert!(bumper.execute("0.2.0").is_err());
    }
}
