use crate::config::{Component, Config};
use crate::pipeline::{Pipeline, PipelineStep, Rollback};
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::ZipWriter;
use zip::write::FileOptions;

pub struct Builder {
    config: Config,
    root: PathBuf,
    tmp_dir: tempfile::TempDir,
}

impl Builder {
    pub fn new(config: Config, root: PathBuf) -> Result<Self> {
        let tmp_dir = tempfile::TempDir::new()?;

        Ok(Self {
            config,
            root,
            tmp_dir,
        })
    }

    pub fn build_all(&self) -> Result<Vec<BuiltArtifact>> {
        let mut artifacts = Vec::new();
        for component in &self.config.components {
            println!("[+] Building {}...", component.name);
            let artifact = self.build_component(component)?;
            artifacts.push(artifact);
        }
        Ok(artifacts)
    }

    fn build_component(&self, component: &Component) -> Result<BuiltArtifact> {
        let component_path = self.root.join(&component.path);

        self.run_build(&component.build, &component_path)
            .with_context(|| format!("Build failed for component '{}'", component.name))?;

        let artifact_path = component_path.join(&component.artifact);

        if !artifact_path.exists() {
            bail!(
                "Artifact not found after build: {}",
                artifact_path.display()
            );
        }

        let paths = if component.zip {
            println!("[+] Archiving {}...", artifact_path.display());
            let zip_name = format!("{}.zip", component.name);
            let zip_path = component_path.join(zip_name);
            Self::zip_dir(&artifact_path, &zip_path)
                .with_context(|| format!("Failed to archive '{}'", artifact_path.display()))?;
            vec![zip_path]
        } else if artifact_path.is_dir() {
            let mut paths = Vec::new();
            for entry in fs::read_dir(&artifact_path)
                .with_context(|| format!("Cannot read artifact dir: {}", artifact_path.display()))?
            {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    paths.push(path);
                } else if path.is_dir() {
                    let zip_name = format!("{}.zip", path.file_name().unwrap().to_string_lossy());
                    let zip_path = artifact_path.join(zip_name);
                    Self::zip_dir(&path, &zip_path)
                        .with_context(|| format!("Failed to archive '{}'", path.display()))?;
                    paths.push(zip_path);
                }
            }
            paths
        } else {
            vec![artifact_path]
        };

        println!("[+] {} built: {} file(s)", component.name, paths.len());

        Ok(BuiltArtifact {
            component_name: component.name.clone(),
            paths,
        })
    }
    fn run_build(&self, cmd: &str, cwd: &Path) -> Result<()> {
        let status = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", cmd])
                .current_dir(cwd)
                .status()
        } else {
            Command::new("sh")
                .args(["-c", cmd])
                .current_dir(cwd)
                .status()
        }
        .with_context(|| format!("Failed to spawn: {cmd}"))?;

        if !status.success() {
            bail!("Command exited with {}: {cmd}", status.code().unwrap_or(-1));
        }
        Ok(())
    }

    fn zip_dir(src: &Path, dst: &Path) -> Result<()> {
        let file = fs::File::create(dst)
            .with_context(|| format!("Cannot create zip: {}", dst.display()))?;

        let mut zip = ZipWriter::new(file);
        let options = FileOptions::<()>::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        if src.is_file() {
            let name = src.file_name().unwrap().to_string_lossy();
            zip.start_file(name, options)?;
            let mut f = fs::File::open(src)?;
            std::io::copy(&mut f, &mut zip)?;
        } else {
            let base = src.parent().unwrap_or(src);
            for entry in walkdir::WalkDir::new(src).min_depth(1) {
                let entry = entry?;
                let path = entry.path();
                let name = path.strip_prefix(base)?;
                if path.is_file() {
                    zip.start_file(name.to_string_lossy(), options)?;
                    let mut f = fs::File::open(path)?;
                    std::io::copy(&mut f, &mut zip)?;
                } else if path.is_dir() {
                    zip.add_directory(name.to_string_lossy(), options)?;
                }
            }
        }

        zip.finish()?;
        Ok(())
    }

    pub fn build_step(self) -> PipelineStep {
        PipelineStep {
            name: "build all components".into(),
            run: Box::new(move |ctx| {
                Box::pin(async move {
                    let artifacts = self.build_all()?;
                    ctx.artifacts = artifacts;
                    Ok(Pipeline::no_rollback())
                })
            }),
            silent: false,
        }
    }
}

pub struct BuiltArtifact {
    pub component_name: String,
    pub paths: Vec<PathBuf>,
}
