use crate::config::{ChangelogType, ReleaseMode};
use crate::pipeline::pipeline::{Pipeline, PipelineStep};
use crate::pipeline::pipeline_steps::github_step::{build_release_body, release_title};
use crate::pipeline::pipeline_steps::interactive_step::{Interactive, PlanAction};
use crate::pipeline::pipeline_steps::versioning_step::VersionBumper;
use crate::pipeline::release_context::render_draft;
use crate::pipeline::{AppContext, PlanStop};
use anyhow::{Context, Result};
use std::path::{Component as PathComponent, Path, PathBuf};

/// A version field change planned for one version file.
pub struct VersionFileChange {
    pub path: PathBuf,
    pub field: Vec<String>,
    pub old: String,
    pub new: String,
}

/// A git commit the release would create.
pub struct PlanCommit {
    pub message: String,
    pub files: Vec<PathBuf>,
}

/// An artifact a component would produce during the build.
pub struct PlanArtifact {
    pub component: String,
    pub build: String,
    pub artifact: String,
    pub zip: bool,
}

/// Everything a release would do, computed without touching disk, git or GitHub.
pub struct ReleasePlan {
    pub version: String,
    pub tag: String,
    pub prev_tag: Option<String>,
    pub branch: String,
    pub changelog_body: Option<String>,
    pub changelog_note: Option<String>,
    pub version_files: Vec<VersionFileChange>,
    pub commits: Vec<PlanCommit>,
    pub artifacts: Vec<PlanArtifact>,
    pub release_mode: ReleaseMode,
    pub sha256sums: bool,
    pub release_title: String,
    pub release_body: String,
    pub repo: String,
}

impl ReleasePlan {
    /// Builds the plan from a context up to and including version selection.
    /// Read-only: no files are written and no external commands are run.
    pub fn assemble(ctx: &AppContext) -> Result<ReleasePlan> {
        let root = ctx.root.as_ref().context("project root not determined")?;
        let config = ctx.config.as_ref().context("config not loaded")?;
        let version = ctx
            .release_version
            .as_ref()
            .context("release version not selected")?
            .clone();
        let branch = ctx.branch.clone().context("branch not resolved")?;
        let prev_tag = ctx.tags.first().cloned();

        let mut changelog_body = None;
        let mut changelog_note = None;
        match config.project.changelog_type {
            ChangelogType::Conventional | ChangelogType::Raw => {
                let draft = render_draft(
                    root,
                    &version,
                    config.project.changelog_type,
                    &config.project.repo,
                )?;
                changelog_body = Some(section_body(&draft));
            }
            ChangelogType::Manual => {
                changelog_note = Some(format!(
                    "the [{version}] section must be added to the changelog by hand; the release body is read from it after that"
                ));
            }
            ChangelogType::None => {
                changelog_note =
                    Some("changelog is disabled, the release body will be empty".to_string());
            }
        }

        let mut version_files = Vec::new();
        let mut bumped_files: Vec<PathBuf> = Vec::new();
        for component in &config.components {
            for vf in &component.version_files {
                let field = vf.resolved_path()?;
                let full_path =
                    clean_path(&root.join(&component.path).join(&vf.file));
                let bumper = VersionBumper::from_file(full_path.clone(), field.clone())?;
                let preview = bumper.preview(&version)?;
                bumped_files.push(full_path.clone());
                let display_path = full_path
                    .strip_prefix(root)
                    .map(|p| p.to_path_buf())
                    .unwrap_or(full_path);
                version_files.push(VersionFileChange {
                    path: display_path,
                    field,
                    old: preview.old,
                    new: preview.new,
                });
            }
        }

        let mut commits = Vec::new();
        if config.project.changelog_type != ChangelogType::None
            && let Some(changelog) = config.project.changelog.as_ref()
        {
            commits.push(PlanCommit {
                message: "chore: update changelog".to_string(),
                files: vec![root.join(changelog)],
            });
        }
        commits.push(PlanCommit {
            message: format!("chore: update app version to {version}"),
            files: bumped_files,
        });

        let artifacts = config
            .components
            .iter()
            .map(|c| PlanArtifact {
                component: c.name.clone(),
                build: c.build.clone(),
                artifact: c.artifact.clone(),
                zip: c.zip,
            })
            .collect();

        let release_title = release_title(&config.project.name, &version);
        let release_body = match &changelog_body {
            Some(body) => {
                build_release_body(body, prev_tag.as_deref(), &version, &config.project.repo)
            }
            None => String::new(),
        };

        let tag = version.clone();
        Ok(ReleasePlan {
            version,
            tag,
            prev_tag,
            branch,
            changelog_body,
            changelog_note,
            version_files,
            commits,
            artifacts,
            release_mode: config.project.release_mode,
            sha256sums: config.project.sha256sums,
            release_title,
            release_body,
            repo: config.project.repo.clone(),
        })
    }
}

/// Removes "." components so a component path of "." does not leak into
/// displayed paths.
fn clean_path(path: &Path) -> PathBuf {
    path.components()
        .filter(|c| !matches!(c, PathComponent::CurDir))
        .collect()
}

/// Extracts the section body from a generated changelog draft, mirroring what
/// the changelog parser reads from the real file.
fn section_body(draft: &str) -> String {
    match draft.split_once("\n\n") {
        Some((_, body)) => body.trim().to_string(),
        None => draft.trim().to_string(),
    }
}

/// Renders the plan as a human-readable report.
pub fn render(plan: &ReleasePlan) -> String {
    let mut out = String::new();
    out.push_str(&format!("Release plan for {}\n", plan.version));
    out.push_str(&format!(
        "  tag: {} (previous tag: {})\n",
        plan.tag,
        plan.prev_tag.as_deref().unwrap_or("none")
    ));
    out.push_str(&format!("  branch: {}\n", plan.branch));

    out.push_str("\nVersion files:\n");
    if plan.version_files.is_empty() {
        out.push_str("  none configured\n");
    }
    for change in &plan.version_files {
        out.push_str(&format!(
            "  {}: {} changes from {} to {}\n",
            change.path.display(),
            change.field.join("."),
            change.old,
            change.new
        ));
    }

    out.push_str("\nChangelog:\n");
    match (&plan.changelog_body, &plan.changelog_note) {
        (Some(body), _) => {
            for line in body.lines() {
                out.push_str(&format!("  {line}\n"));
            }
        }
        (None, Some(note)) => out.push_str(&format!("  {note}\n")),
        (None, None) => out.push_str("  none\n"),
    }

    out.push_str("\nCommits:\n");
    for commit in &plan.commits {
        out.push_str(&format!("  {}\n", commit.message));
        for file in &commit.files {
            out.push_str(&format!("    {}\n", file.display()));
        }
    }

    out.push_str("\nArtifacts:\n");
    match plan.release_mode {
        ReleaseMode::Ci => {
            out.push_str("  release_mode is ci: build and GitHub release are handled by CI after the tag is pushed\n");
        }
        ReleaseMode::Local => {
            out.push_str("  names appear only after the build runs\n");
            for artifact in &plan.artifacts {
                out.push_str(&format!("  component {}:\n", artifact.component));
                out.push_str(&format!(
                    "    build: {}\n",
                    if artifact.build.is_empty() {
                        "(none)"
                    } else {
                        &artifact.build
                    }
                ));
                out.push_str(&format!("    artifact: {}\n", artifact.artifact));
                out.push_str(&format!(
                    "    zip: {}\n",
                    if artifact.zip { "yes" } else { "no" }
                ));
            }
        }
    }

    out.push_str("\nGitHub release:\n");
    match plan.release_mode {
        ReleaseMode::Ci => out
            .push_str("  release_mode is ci: the release is created by CI, not by jrit\n"),
        ReleaseMode::Local => {
            out.push_str(&format!("  repo: {}\n", plan.repo));
            out.push_str(&format!("  title: {}\n", plan.release_title));
            out.push_str(&format!(
                "  checksum file: {}\n",
                if plan.sha256sums {
                    "sha256sums.txt"
                } else {
                    "disabled"
                }
            ));
            out.push_str("  body:\n");
            if plan.release_body.is_empty() {
                out.push_str("    (empty)\n");
            } else {
                for line in plan.release_body.lines() {
                    out.push_str(&format!("    {line}\n"));
                }
            }
        }
    }

    out
}

pub fn step() -> PipelineStep {
    PipelineStep::new("release plan", false, |ctx| {
        Box::pin(async move {
            let plan = ReleasePlan::assemble(ctx)?;
            println!("{}", render(&plan));
            if ctx.dry_run {
                return Err(PlanStop {
                    message: "dry run complete, no changes were made",
                }
                .into());
            }
            match Interactive::confirm_plan()? {
                PlanAction::Apply => Ok(Pipeline::no_rollback()),
                PlanAction::Cancel => Err(PlanStop {
                    message: "release cancelled, no changes were made",
                }
                .into()),
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Component, Config, Project, VersionFile};
    use std::path::Path;

    fn git(root: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn make_ctx(root: &Path) -> AppContext {
        let mut ctx = AppContext::new();
        ctx.root = Some(root.to_path_buf());
        ctx.config = Some(Config {
            project: Project {
                name: "demo".into(),
                repo: "owner/demo".into(),
                branches: vec!["main".into()],
                changelog_type: ChangelogType::Conventional,
                changelog: Some("CHANGELOG.md".into()),
                release_mode: ReleaseMode::Local,
                sha256sums: true,
            },
            components: vec![Component {
                name: "demo".into(),
                path: ".".into(),
                build: "cargo build --release".into(),
                artifact: "./target/release/demo".into(),
                zip: true,
                version_files: vec![VersionFile {
                    file: "Cargo.toml".into(),
                    path: None,
                }],
            }],
        });
        ctx.tags = vec!["v1.0.0".into()];
        ctx.release_version = Some("1.1.0".into());
        ctx.branch = Some("main".into());
        ctx
    }

    fn make_repo(root: &Path) {
        git(root, &["init", "-q"]);
        git(root, &["config", "user.email", "t@t"]);
        git(root, &["config", "user.name", "t"]);
        std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n")
            .unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "feat: first"]);
        git(root, &["tag", "v1.0.0"]);
        std::fs::write(root.join("f.txt"), "2").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "feat: second"]);
    }

    #[test]
    fn assemble_collects_changes_commits_and_tag() {
        let dir = tempfile::tempdir().unwrap();
        make_repo(dir.path());
        let ctx = make_ctx(dir.path());

        let plan = ReleasePlan::assemble(&ctx).unwrap();

        assert_eq!(plan.version, "1.1.0");
        assert_eq!(plan.tag, "1.1.0");
        assert_eq!(plan.prev_tag.as_deref(), Some("v1.0.0"));
        assert_eq!(plan.branch, "main");

        assert_eq!(plan.version_files.len(), 1);
        let change = &plan.version_files[0];
        assert_eq!(change.path, PathBuf::from("Cargo.toml"));
        assert_eq!(change.field, vec!["package", "version"]);
        assert_eq!(change.old, "0.1.0");
        assert_eq!(change.new, "1.1.0");

        assert_eq!(plan.commits.len(), 2);
        assert_eq!(plan.commits[0].message, "chore: update changelog");
        assert_eq!(plan.commits[0].files, vec![dir.path().join("CHANGELOG.md")]);
        assert_eq!(
            plan.commits[1].message,
            "chore: update app version to 1.1.0"
        );
        assert_eq!(plan.commits[1].files, vec![dir.path().join("Cargo.toml")]);

        assert!(plan.release_body.contains("1.1.0"));
        assert!(plan
            .release_body
            .contains("https://github.com/owner/demo/compare/v1.0.0...1.1.0"));
        assert_eq!(plan.release_title, "demo 1.1.0");
    }

    #[test]
    fn assemble_makes_no_changes_to_the_repo() {
        let dir = tempfile::tempdir().unwrap();
        make_repo(dir.path());
        let before = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
        let ctx = make_ctx(dir.path());

        let plan = ReleasePlan::assemble(&ctx).unwrap();
        render(&plan);

        assert_eq!(
            std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap(),
            before
        );
        assert!(!dir.path().join("CHANGELOG.md").exists());
    }

    #[test]
    fn render_shows_all_sections() {
        let dir = tempfile::tempdir().unwrap();
        make_repo(dir.path());
        let ctx = make_ctx(dir.path());
        let plan = ReleasePlan::assemble(&ctx).unwrap();

        let out = render(&plan);
        assert!(out.contains("Release plan for 1.1.0"));
        assert!(out.contains("tag: 1.1.0 (previous tag: v1.0.0)"));
        assert!(out.contains("Cargo.toml: package.version changes from 0.1.0 to 1.1.0"));
        assert!(out.contains("chore: update changelog"));
        assert!(out.contains("chore: update app version to 1.1.0"));
        assert!(out.contains("component demo:"));
        assert!(out.contains("build: cargo build --release"));
        assert!(out.contains("checksum file: sha256sums.txt"));
        assert!(out.contains("title: demo 1.1.0"));
    }

    #[test]
    fn render_notes_ci_mode() {
        let dir = tempfile::tempdir().unwrap();
        make_repo(dir.path());
        let mut ctx = make_ctx(dir.path());
        ctx.config.as_mut().unwrap().project.release_mode = ReleaseMode::Ci;
        let plan = ReleasePlan::assemble(&ctx).unwrap();

        let out = render(&plan);
        assert!(out.contains("release_mode is ci"));
        assert!(!out.contains("component demo:"));
    }

    #[test]
    fn section_body_strips_heading() {
        assert_eq!(
            section_body("## [1.1.0] - 2024-05-01\n\n### Features\n- add thing\n"),
            "### Features\n- add thing"
        );
        assert_eq!(section_body("no heading"), "no heading");
    }
}
