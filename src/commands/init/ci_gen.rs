use indexmap::IndexMap;
use serde::Serialize;

#[derive(Serialize)]
pub struct Workflow {
    pub name: String,
    pub on: On,
    pub jobs: IndexMap<String, Job>,
}

#[derive(Serialize)]
pub struct On {
    pub push: PushTrigger,
}

#[derive(Serialize)]
pub struct PushTrigger {
    pub tags: Vec<String>,
}

#[derive(Serialize)]
pub struct Job {
    pub strategy: Strategy,
    #[serde(rename = "runs-on")]
    pub runs_on: String,
    pub steps: Vec<Step>,
}

#[derive(Serialize)]
pub struct Strategy {
    pub matrix: Matrix,
}

#[derive(Serialize)]
pub struct Matrix {
    pub include: Vec<MatrixEntry>,
}

#[derive(Serialize)]
pub struct MatrixEntry {
    pub os: String,
    pub target: String,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum Step {
    Uses(UsesStep),
    Run(RunStep),
}

#[derive(Serialize)]
pub struct UsesStep {
    pub name: String,
    pub uses: String,
    #[serde(rename = "if", skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub with: Option<IndexMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<IndexMap<String, String>>,
}

#[derive(Serialize)]
pub struct RunStep {
    pub name: String,
    #[serde(rename = "if", skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    pub run: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SetupStep {
    RustToolchain,
    Bun,
    Node,
    CargoCache,
}

impl SetupStep {
    pub fn label(&self) -> &str {
        match self {
            Self::RustToolchain => "Rust toolchain",
            Self::Bun => "Bun",
            Self::Node => "Node.js",
            Self::CargoCache => "Cargo cache",
        }
    }

    fn to_workflow_steps(&self) -> Vec<Step> {
        match self {
            Self::RustToolchain => vec![
                Step::Run(RunStep {
                    name: "Install Rust targets (macOS universal)".into(),
                    condition: Some("matrix.os == 'macos-latest'".into()),
                    run: "rustup target add x86_64-apple-darwin aarch64-apple-darwin".into(),
                }),
                Step::Uses(UsesStep {
                    name: "Install Rust toolchain".into(),
                    uses: "dtolnay/rust-toolchain@stable".into(),
                    condition: Some("matrix.os != 'macos-latest'".into()),
                    with: None,
                    env: None,
                }),
            ],
            Self::Bun => vec![Step::Uses(UsesStep {
                name: "Setup Bun".into(),
                uses: "oven-sh/setup-bun@v2".into(),
                condition: None,
                with: None,
                env: None,
            })],
            Self::Node => vec![Step::Uses(UsesStep {
                name: "Setup Node.js".into(),
                uses: "actions/setup-node@v4".into(),
                condition: None,
                with: Some({
                    let mut m = IndexMap::new();
                    m.insert("node-version".into(), "lts/*".into());
                    m
                }),
                env: None,
            })],
            Self::CargoCache => vec![Step::Uses(UsesStep {
                name: "Cache Cargo".into(),
                uses: "actions/cache@v4".into(),
                condition: None,
                with: Some({
                    let mut m = IndexMap::new();
                    m.insert("path".into(), "~/.cargo/registry\n~/.cargo/git".into());
                    m.insert(
                        "key".into(),
                        "${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}".into(),
                    );
                    m.insert("restore-keys".into(), "${{ runner.os }}-cargo-".into());
                    m
                }),
                env: None,
            })],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Target {
    pub os: &'static str,
    pub target: &'static str,
    pub label: &'static str,
}

pub fn available_targets() -> Vec<Target> {
    vec![
        Target {
            os: "macos-latest",
            target: "universal-apple-darwin",
            label: "macOS (universal)",
        },
        Target {
            os: "windows-latest",
            target: "x86_64-pc-windows-msvc",
            label: "Windows (x86_64)",
        },
        Target {
            os: "ubuntu-latest",
            target: "x86_64-unknown-linux-gnu",
            label: "Linux (x86_64)",
        },
    ]
}

pub struct CiGenParams {
    pub project_name: String,
    pub build_command: String,
    pub artifact_name: String,
    pub targets: Vec<Target>,
    pub setup_steps: Vec<SetupStep>,
}

pub fn generate_workflow(params: CiGenParams) -> anyhow::Result<String> {
    let has_mac = params.targets.iter().any(|t| t.os == "macos-latest");

    let mut steps: Vec<Step> = vec![Step::Uses(UsesStep {
        name: "Checkout".into(),
        uses: "actions/checkout@v4".into(),
        condition: None,
        with: None,
        env: None,
    })];

    for setup in &params.setup_steps {
        steps.extend(setup.to_workflow_steps());
    }

    if has_mac {
        steps.push(Step::Run(RunStep {
            name: "Build (macOS universal)".into(),
            condition: Some("matrix.os == 'macos-latest'".into()),
            run: format!(
                "{cmd} --target x86_64-apple-darwin\n\
                 {cmd} --target aarch64-apple-darwin\n\
                 lipo -create -output {artifact} \
                   target/x86_64-apple-darwin/release/{artifact} \
                   target/aarch64-apple-darwin/release/{artifact}",
                cmd = params.build_command,
                artifact = params.artifact_name,
            ),
        }));
        steps.push(Step::Run(RunStep {
            name: "Build".into(),
            condition: Some("matrix.os != 'macos-latest'".into()),
            run: params.build_command.clone(),
        }));
    } else {
        steps.push(Step::Run(RunStep {
            name: "Build".into(),
            condition: None,
            run: params.build_command.clone(),
        }));
    }

    steps.push(Step::Uses(UsesStep {
        name: "Upload artifact (Windows)".into(),
        uses: "actions/upload-artifact@v4".into(),
        condition: Some("matrix.os == 'windows-latest'".into()),
        with: Some({
            let mut m = IndexMap::new();
            m.insert("name".into(), "${{ matrix.os }}-artifact".into());
            m.insert("path".into(), format!("{}.exe", params.artifact_name));
            m
        }),
        env: None,
    }));
    steps.push(Step::Uses(UsesStep {
        name: "Upload artifact".into(),
        uses: "actions/upload-artifact@v4".into(),
        condition: Some("matrix.os != 'windows-latest'".into()),
        with: Some({
            let mut m = IndexMap::new();
            m.insert("name".into(), "${{ matrix.os }}-artifact".into());
            m.insert("path".into(), params.artifact_name.clone());
            m
        }),
        env: None,
    }));

    let matrix_entries = params
        .targets
        .iter()
        .map(|t| MatrixEntry {
            os: t.os.to_string(),
            target: t.target.to_string(),
        })
        .collect();

    let mut jobs = IndexMap::new();
    jobs.insert(
        "build".to_string(),
        Job {
            strategy: Strategy {
                matrix: Matrix {
                    include: matrix_entries,
                },
            },
            runs_on: "${{ matrix.os }}".into(),
            steps,
        },
    );

    let workflow = Workflow {
        name: format!("Release {}", params.project_name),
        on: On {
            push: PushTrigger {
                tags: vec!["*".into()],
            },
        },
        jobs,
    };

    Ok(serde_yaml::to_string(&workflow)?)
}

pub fn write_workflow(content: &str) -> anyhow::Result<()> {
    let dir = std::path::Path::new("../../../.github/workflows");
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("release.yml"), content)?;
    Ok(())
}
