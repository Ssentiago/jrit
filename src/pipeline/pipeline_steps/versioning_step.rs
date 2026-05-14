use crate::pipeline::{PipelineStep, Rollback};
use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use std::path::PathBuf;

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

        let mut toml_data: toml::Value = toml::from_str(&content)
            .with_context(|| format!("failed to parse TOML: {}", self.relative_path.display()))?;

        let mut cur = &mut toml_data;
        for field in &self.fields {
            cur = cur.get_mut(field.as_str()).with_context(|| {
                format!(
                    "field '{}' not found in {}",
                    field,
                    self.relative_path.display()
                )
            })?;
        }
        *cur = toml::Value::String(new_version.to_string());

        let updated = toml::to_string_pretty(&toml_data).with_context(|| {
            format!("failed to serialize TOML: {}", self.relative_path.display())
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
