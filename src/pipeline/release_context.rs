use crate::config::ChangelogType;
use crate::pipeline::pipeline::{Pipeline, PipelineStep};
use anyhow::{Context, Result, bail};
use git_cliff_core::{
    changelog::Changelog,
    commit::Commit,
    config::{ChangelogConfig, CommitParser, Config, GitConfig},
    release::Release,
    repo::Repository,
};
use regex::Regex;
use semver::Version;
use std::path::Path;

/// Conventional commit classification used for the release recommendation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitKind {
    Breaking,
    Feature,
    Fix,
    Other,
}

impl CommitKind {
    pub fn label(self) -> &'static str {
        match self {
            CommitKind::Breaking => "breaking",
            CommitKind::Feature => "feature",
            CommitKind::Fix => "fix",
            CommitKind::Other => "other",
        }
    }
}

/// Recommended semantic version bump level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BumpLevel {
    Major,
    Minor,
    Patch,
}

impl BumpLevel {
    pub fn label(self) -> &'static str {
        match self {
            BumpLevel::Major => "Major",
            BumpLevel::Minor => "Minor",
            BumpLevel::Patch => "Patch",
        }
    }
}

/// A single commit summarized for display in the version menu.
pub struct CommitSummary {
    pub id: String,
    pub subject: String,
    pub kind: Option<CommitKind>,
}

impl CommitSummary {
    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            Some(k) => k.label(),
            None => "unconventional",
        }
    }
}

/// Commits since the last release tag plus the derived bump recommendation.
pub struct ReleaseAnalysis {
    pub prev_tag: Option<String>,
    pub commits: Vec<CommitSummary>,
    pub breaking: usize,
    pub feature: usize,
    pub fix: usize,
    pub recommendation: Option<BumpLevel>,
}

impl ReleaseAnalysis {
    pub fn is_first_release(&self) -> bool {
        self.prev_tag.is_none()
    }

    pub fn is_empty(&self) -> bool {
        self.commits.is_empty()
    }

    pub fn other(&self) -> usize {
        self.commits
            .len()
            .saturating_sub(self.breaking + self.feature + self.fix)
    }

    pub fn recommended_version(&self, current: &str) -> Option<String> {
        let level = self.recommendation?;
        let v = parse_version_tag(current)?;
        let [patch, minor, major] = bump_strings(v.major, v.minor, v.patch);
        Some(match level {
            BumpLevel::Patch => patch,
            BumpLevel::Minor => minor,
            BumpLevel::Major => major,
        })
    }

    pub fn preview_version(&self, current: &str) -> Option<String> {
        if self.is_first_release() {
            return None;
        }
        let v = parse_version_tag(current)?;
        let [patch, minor, major] = bump_strings(v.major, v.minor, v.patch);
        Some(match self.recommendation {
            Some(BumpLevel::Patch) | None => patch,
            Some(BumpLevel::Minor) => minor,
            Some(BumpLevel::Major) => major,
        })
    }
}

pub fn parse_version_tag(tag: &str) -> Option<Version> {
    let trimmed = tag.trim();
    let stripped = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'))
        .unwrap_or(trimmed);
    Version::parse(stripped).ok()
}

/// Returns `[patch, minor, major]` next version strings for a parsed version.
pub fn bump_strings(major: u64, minor: u64, patch: u64) -> [String; 3] {
    [
        format!("{major}.{minor}.{}", patch + 1),
        format!("{major}.{}.0", minor + 1),
        format!("{}.0.0", major + 1),
    ]
}

/// Splits a conventional commit header, returning `(type, breaking_marker)`.
fn split_conventional(message: &str) -> Option<(String, bool)> {
    let header = message.lines().next().unwrap_or("").trim_end();
    let colon = header.find(':')?;
    let before = header[..colon].trim_end();
    let bang = before.ends_with('!');
    let before = before.trim_end_matches('!');
    let type_part = match before.find('(') {
        Some(i) => &before[..i],
        None => before,
    };
    let type_part = type_part.trim();
    if type_part.is_empty() || !type_part.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    Some((type_part.to_ascii_lowercase(), bang))
}

fn has_breaking_footer(message: &str) -> bool {
    message.lines().skip(1).any(|l| {
        let t = l.trim_start();
        t.starts_with("BREAKING CHANGE:") || t.starts_with("BREAKING-CHANGE:")
    })
}

/// Classifies a commit message, returning `None` when it is not conventional.
fn classify(message: &str) -> Option<CommitKind> {
    let (ctype, bang) = split_conventional(message)?;
    let kind = if bang || has_breaking_footer(message) {
        CommitKind::Breaking
    } else {
        match ctype.as_str() {
            "feat" => CommitKind::Feature,
            "fix" => CommitKind::Fix,
            _ => CommitKind::Other,
        }
    };
    Some(kind)
}

fn previous_release_tag(root: &Path) -> Result<Option<String>> {
    let output = std::process::Command::new("git")
        .args(["tag", "-l", "--merged", "HEAD"])
        .current_dir(root)
        .output()
        .context("failed to list git tags")?;

    if !output.status.success() {
        bail!(
            "git tag failed with {}\nstderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let tags = String::from_utf8(output.stdout)?;
    Ok(tags
        .lines()
        .filter_map(|tag| parse_version_tag(tag).map(|version| (version, tag)))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, tag)| tag.to_string()))
}

/// Reads commits since the last release tag once and derives the recommendation.
pub fn analyze(root: &Path) -> Result<ReleaseAnalysis> {
    let prev_tag = previous_release_tag(root)?;
    let repo = Repository::init(root.to_path_buf())?;
    let range = prev_tag.as_ref().map(|tag| format!("{tag}..HEAD"));
    let git_commits = repo.commits(range.as_deref(), None, None, false)?;

    let mut commits: Vec<CommitSummary> = Vec::new();
    let mut breaking = 0usize;
    let mut feature = 0usize;
    let mut fix = 0usize;

    for git_commit in &git_commits {
        let parsed = Commit::from(git_commit);
        let message = parsed.message.clone();
        let id: String = parsed.id.chars().take(7).collect();
        let kind = classify(&message);
        match kind {
            Some(CommitKind::Breaking) => breaking += 1,
            Some(CommitKind::Feature) => feature += 1,
            Some(CommitKind::Fix) => fix += 1,
            _ => {}
        }
        let subject = message.lines().next().unwrap_or("").to_string();
        commits.push(CommitSummary { id, subject, kind });
    }

    let any_conventional = commits.iter().any(|c| c.kind.is_some());
    let recommendation = derive_recommendation(prev_tag.is_none(), &commits, any_conventional);

    Ok(ReleaseAnalysis {
        prev_tag,
        commits,
        breaking,
        feature,
        fix,
        recommendation,
    })
}

/// Derives the bump recommendation from classified commits.
///
/// Returns `None` on a first release, when there is nothing to release, or when
/// no commit is conventional and therefore unclassifiable.
fn derive_recommendation(
    is_first_release: bool,
    commits: &[CommitSummary],
    any_conventional: bool,
) -> Option<BumpLevel> {
    if is_first_release || commits.is_empty() || !any_conventional {
        return None;
    }
    if commits.iter().any(|c| c.kind == Some(CommitKind::Breaking)) {
        return Some(BumpLevel::Major);
    }
    if commits.iter().any(|c| c.kind == Some(CommitKind::Feature)) {
        return Some(BumpLevel::Minor);
    }
    Some(BumpLevel::Patch)
}

/// Renders the changelog draft for a given version, shared by the menu preview
/// and the real changelog generation so the two never drift apart.
pub fn render_draft(
    root: &Path,
    version: &str,
    changelog_type: ChangelogType,
    repo_path: &str,
) -> Result<String> {
    match changelog_type {
        ChangelogType::Conventional => generate_conventional(root, version, repo_path),
        ChangelogType::Raw => generate_raw(root, version),
        other => bail!("changelog_type {other:?} does not generate a changelog"),
    }
}

fn generate_conventional(root: &Path, version: &str, repo_path: &str) -> Result<String> {
    let repo = Repository::init(root.to_path_buf())?;

    let body_template = "\
{% set order = [\"Features\", \"Bug Fixes\", \"Refactoring\", \"Chores\"] %}\
{% for group_name in order %}\
{% set group_commits = commits | filter(attribute=\"group\", value=group_name) %}\
{% if group_commits %}\
### {{ group_name }}\n\
{% for commit in group_commits %}- {{ commit.message }} ([`{{ commit.id | truncate(length=7, end=\"\") }}`](https://github.com/__REPO__/commit/{{ commit.id }}))\n{% endfor %}\n\
{% endif %}\
{% endfor %}"
        .replace("__REPO__", repo_path);

    let config = Config {
        changelog: ChangelogConfig {
            header: None,
            body: body_template,
            footer: None,
            trim: true,
            ..Default::default()
        },
        git: GitConfig {
            conventional_commits: true,
            filter_unconventional: false,
            commit_parsers: vec![
                CommitParser {
                    message: Some(Regex::new("^feat").unwrap()),
                    group: Some("Features".into()),
                    ..Default::default()
                },
                CommitParser {
                    message: Some(Regex::new("^fix").unwrap()),
                    group: Some("Bug Fixes".into()),
                    ..Default::default()
                },
                CommitParser {
                    message: Some(Regex::new("^refactor").unwrap()),
                    group: Some("Refactoring".into()),
                    ..Default::default()
                },
                CommitParser {
                    message: Some(Regex::new("^chore").unwrap()),
                    group: Some("Chores".into()),
                    skip: Some(true),
                    ..Default::default()
                },
            ],
            ..Default::default()
        },
        remote: Default::default(),
        bump: Default::default(),
    };

    let range = previous_release_tag(root)?.map(|tag| format!("{tag}..HEAD"));

    let git_commits = repo.commits(range.as_deref(), None, None, false)?;
    let commits: Vec<Commit> = git_commits.iter().map(Commit::from).collect();

    let release = Release {
        version: Some(version.to_string()),
        commits,
        ..Default::default()
    };

    let changelog = Changelog::new(vec![release], config, range.as_deref())?;
    let mut out = Vec::new();
    changelog.generate(&mut out)?;

    let body = String::from_utf8(out)?;
    let date = chrono::Local::now().format("%Y-%m-%d");
    Ok(format!("## [{version}] - {date}\n\n{body}"))
}

fn generate_raw(root: &Path, version: &str) -> Result<String> {
    let range_arg = previous_release_tag(root)?
        .map(|tag| format!("{tag}..HEAD"))
        .unwrap_or_else(|| "HEAD".to_string());

    let output = std::process::Command::new("git")
        .args(["log", "--oneline", "--no-decorate", &range_arg])
        .current_dir(root)
        .output()
        .context("failed to run git log")?;

    let log = String::from_utf8(output.stdout)?;
    let date = chrono::Local::now().format("%Y-%m-%d");
    Ok(format!("## [{version}] - {date}\n\n{log}"))
}

pub fn collect_step() -> PipelineStep {
    PipelineStep::new("collect release context", true, |ctx| {
        Box::pin(async move {
            let root = ctx
                .root
                .as_ref()
                .context("project root not determined")?
                .clone();
            let analysis = analyze(&root)?;
            ctx.release_analysis = Some(analysis);
            Ok(Pipeline::no_rollback())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_tag_accepts_prefixes_and_plain() {
        assert_eq!(parse_version_tag("v1.2.3").unwrap().to_string(), "1.2.3");
        assert_eq!(parse_version_tag("V2.0.0").unwrap().to_string(), "2.0.0");
        assert_eq!(parse_version_tag("0.1.0").unwrap().to_string(), "0.1.0");
        assert_eq!(parse_version_tag(" 1.0.0 ").unwrap().to_string(), "1.0.0");
    }

    #[test]
    fn parse_version_tag_rejects_invalid() {
        assert!(parse_version_tag("not-a-version").is_none());
        assert!(parse_version_tag("1.2").is_none());
        assert!(parse_version_tag("").is_none());
    }

    #[test]
    fn classify_conventional_types() {
        assert_eq!(classify("feat: add thing"), Some(CommitKind::Feature));
        assert_eq!(classify("fix(api): patch bug"), Some(CommitKind::Fix));
        assert_eq!(classify("chore: tidy up"), Some(CommitKind::Other));
        assert_eq!(classify("docs: write readme"), Some(CommitKind::Other));
        assert_eq!(classify("refactor: move code"), Some(CommitKind::Other));
    }

    #[test]
    fn classify_breaking_variants() {
        assert_eq!(classify("feat!: drop old api"), Some(CommitKind::Breaking));
        assert_eq!(classify("fix(x)!: break it"), Some(CommitKind::Breaking));
        assert_eq!(
            classify("feat: change\n\nBREAKING CHANGE: removed field"),
            Some(CommitKind::Breaking)
        );
    }

    #[test]
    fn classify_unconventional_is_none() {
        assert_eq!(classify("just a message"), None);
        assert_eq!(classify("Update stuff"), None);
        assert_eq!(classify(""), None);
    }

    #[test]
    fn recommendation_precedence_major_minor_patch() {
        let mk = |kinds: Vec<Option<CommitKind>>| {
            let commits: Vec<CommitSummary> = kinds
                .into_iter()
                .map(|k| CommitSummary {
                    id: "abc1234".into(),
                    subject: "s".into(),
                    kind: k,
                })
                .collect();
            let any_conventional = commits.iter().any(|c| c.kind.is_some());
            derive_recommendation(false, &commits, any_conventional)
        };

        assert_eq!(
            mk(vec![
                Some(CommitKind::Fix),
                Some(CommitKind::Feature),
                Some(CommitKind::Breaking)
            ]),
            Some(BumpLevel::Major)
        );
        assert_eq!(
            mk(vec![Some(CommitKind::Fix), Some(CommitKind::Feature)]),
            Some(BumpLevel::Minor)
        );
        assert_eq!(mk(vec![Some(CommitKind::Fix)]), Some(BumpLevel::Patch));
    }

    #[test]
    fn recommendation_handles_edge_cases() {
        // empty commits: nothing to release
        assert_eq!(derive_recommendation(false, &[], false), None);

        // only unconventional commits: unclassifiable
        let unconventional = vec![CommitSummary {
            id: "abc1234".into(),
            subject: "random".into(),
            kind: None,
        }];
        assert_eq!(
            derive_recommendation(false, &unconventional, false),
            None
        );

        // only conventional chores and docs: patch
        let chore_only = vec![CommitSummary {
            id: "abc1234".into(),
            subject: "chore".into(),
            kind: Some(CommitKind::Other),
        }];
        assert_eq!(
            derive_recommendation(false, &chore_only, true),
            Some(BumpLevel::Patch)
        );

        // first release: no recommendation regardless of kinds
        let feature = vec![CommitSummary {
            id: "abc1234".into(),
            subject: "feat".into(),
            kind: Some(CommitKind::Feature),
        }];
        assert_eq!(derive_recommendation(true, &feature, true), None);
    }

    #[test]
    fn analyze_reads_commits_since_last_tag() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let git = |args: &[&str]| {
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
        };

        git(&["init", "-q"]);
        git(&["config", "user.email", "t@t"]);
        git(&["config", "user.name", "t"]);
        std::fs::write(root.join("f.txt"), "1").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "feat: first"]);
        git(&["tag", "v1.0.0"]);
        std::fs::write(root.join("f.txt"), "2").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "fix: bug"]);
        std::fs::write(root.join("f.txt"), "3").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "chore: tidy"]);

        let a = analyze(root).unwrap();
        assert_eq!(a.prev_tag.as_deref(), Some("v1.0.0"));
        assert_eq!(a.commits.len(), 2);
        assert_eq!(a.recommendation, Some(BumpLevel::Patch));

        std::fs::write(root.join("f.txt"), "4").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "feat!: break api\n\nBREAKING CHANGE: removed endpoint"]);
        let b = analyze(root).unwrap();
        assert_eq!(b.commits.len(), 3);
        assert_eq!(b.recommendation, Some(BumpLevel::Major));
    }
}
