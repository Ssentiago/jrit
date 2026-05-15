use crate::commands::init::ci_gen::{self, CiGenParams, SetupStep, available_targets};
use crate::config::{Component, Config, Project, VersionFile};
use inquire::MultiSelect;
use inquire::validator::Validation;
use inquire::{Confirm, Select, Text};
use owo_colors::OwoColorize;

pub async fn run_init() -> anyhow::Result<()> {
    let name = Text::new("Project name:").prompt()?;
    let mut repo = Text::new("GitHub repo (owner/repo):");
    repo.validators = vec![Box::new(|v: &str| {
        if v.split("/").count() == 2 {
            Ok(Validation::Valid)
        } else {
            Ok(Validation::Invalid("must be in format 'owner/repo'".into()))
        }
    })];
    let repo = repo.prompt()?;
    let branches = Text::new("Allowed branches (comma-separated):")
        .with_default("main,master")
        .prompt()?;
    let branches: Vec<String> = branches.split(',').map(|s| s.trim().to_string()).collect();

    let changelog_type = Select::new(
        "Changelog type:",
        vec!["none", "conventional", "raw", "manual"],
    )
    .prompt()?;

    let changelog = if changelog_type != "none" {
        Some(
            Text::new("Changelog path:")
                .with_default("CHANGELOG.md")
                .prompt()?,
        )
    } else {
        None
    };

    let release_mode = Select::new("Release mode:", vec!["local", "ci"]).prompt()?;
    let mut components: Vec<Component> = vec![];

    let is_local_mode = release_mode == "local";

    loop {
        println!("Add a component ({} so far)", components.len());
        let name = Text::new("  Component name:").prompt()?;
        let path = Text::new("  Path (relative to jrit.toml):")
            .with_default(".")
            .prompt()?;

        let (build, artifact, zip) = if is_local_mode {
            let build = Text::new("  Build command:").prompt()?;
            let artifact = Text::new("  Artifact path:").prompt()?;
            let zip = Confirm::new("  Zip artifact?")
                .with_default(true)
                .prompt()?;
            (build, artifact, zip)
        } else {
            (String::new(), String::new(), false)
        };

        let mut version_files: Vec<VersionFile> = vec![];
        loop {
            let file = Text::new("    Version file (e.g. Cargo.toml):").prompt()?;
            let infer = Confirm::new("    Infer version path automatically?")
                .with_default(true)
                .prompt()?;
            let path = if infer {
                None
            } else {
                let raw =
                    Text::new("    Version path (comma-separated keys, e.g. package,version):")
                        .prompt()?;
                Some(raw.split(',').map(|k| k.trim().to_string()).collect())
            };

            version_files.push(VersionFile { file, path });

            if !Confirm::new("    Add another version file?")
                .with_default(false)
                .prompt()?
            {
                break;
            }
        }

        components.push(Component {
            name,
            path,
            build,
            artifact,
            zip,
            version_files,
        });

        if !Confirm::new("Add another component?")
            .with_default(false)
            .prompt()?
        {
            break;
        }
    }

    let mut out = Config {
        project: Project {
            name,
            repo,
            branches,
            changelog_type: changelog_type.to_string(),
            release_mode: release_mode.to_string(),
            changelog,
        },
        components,
    };

    std::fs::write("jrit.toml", &toml::to_string(&out)?)?;

    println!("{} jrit.toml created", "✓".green());

    if release_mode == "ci" {
        let generate_ci = Confirm::new("Generate GitHub Actions workflow?")
            .with_default(true)
            .prompt()?;
        if generate_ci {
            let all_targets = available_targets();
            let target_labels: Vec<&str> = all_targets.iter().map(|t| t.label).collect();

            let selected_labels = MultiSelect::new("Select targets:", target_labels)
                .with_default(&[0, 1, 2])
                .prompt()?;

            let targets: Vec<_> = all_targets
                .into_iter()
                .filter(|t| selected_labels.contains(&t.label))
                .collect();

            let setup_options = vec![
                SetupStep::RustToolchain,
                SetupStep::Bun,
                SetupStep::Node,
                SetupStep::CargoCache,
            ];
            let setup_labels: Vec<&str> = setup_options.iter().map(|s| s.label()).collect();

            let selected_setup_labels: Vec<String> =
                MultiSelect::new("Select setup steps:", setup_labels)
                    .prompt()?
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect();

            let setup_steps: Vec<SetupStep> = setup_options
                .into_iter()
                .filter(|s| selected_setup_labels.contains(&s.label().to_string()))
                .collect();

            let default_build = out
                .components
                .first()
                .map(|c| c.build.as_str())
                .unwrap_or("cargo build --release");
            let default_artifact = out
                .components
                .first()
                .map(|c| c.artifact.as_str())
                .unwrap_or("target/release/");

            let build_command = Text::new("Build command:")
                .with_default(default_build)
                .prompt()?;

            let artifact_name = out
                .components
                .first()
                .map(|c| {
                    std::path::Path::new(&c.artifact)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                })
                .unwrap_or("app".to_string());

            let content = ci_gen::generate_workflow(CiGenParams {
                project_name: out.project.name.clone(),
                build_command,
                artifact_name,
                targets,
                setup_steps,
            })?;

            ci_gen::write_workflow(&content)?;
            println!("{} .github/workflows/release.yml created", "✓".green());
            println!(
                "{} This workflow is not production-ready. You need to supplement it to meet the requirements of your project.",
                "Important!".red()
            );
        }
    }

    Ok(())
}
