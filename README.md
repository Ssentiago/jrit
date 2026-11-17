# jrit

> Just Release it


A CLI tool for automating releases: bumps versions, builds artifacts, commits, tags, and publishes a GitHub release —
all in one command.

## Requirements

- Git installed and configured
- `GITHUB_TOKEN` environment variable set with repo write access, or `gh` CLI authenticated (`gh auth login`)
- The repo must have at least one `jrit.toml` at the root folder (jrit walks up from cwd to find it)

## Installation

Download the latest binary from [Releases](https://github.com/Ssentiago/jrit/releases) and put it somewhere in your
`PATH`.

## Usage

Run from anywhere inside your repo:

```bash
jrit
```

jrit will walk up the directory tree to find `jrit.toml` automatically.

### Commands

```bash
jrit init      # interactively create jrit.toml and optionally generate a CI workflow
jrit doctor    # run preflight checks and exit with the result
jrit update    # update jrit itself to the latest version
jrit --dry-run # print the full release plan and exit without making any changes
```

### Dry run

`jrit --dry-run` runs the read-only part of the pipeline and prints the release plan: the version changes for every
configured version file, the git commits that would be created, the tag, the changelog draft, the artifacts and the
GitHub release title and body. Nothing is written, no git state is changed, no editor opens, and the process exits
with code 0.

Version selection stays interactive so you can inspect the plan for any version you pick. Preflight checks run first
and their problems are reported without stopping the plan.

In a normal run the preflight checks must all pass before anything else happens, and the same plan is printed as a
confirmation screen after version selection. Choosing `Apply` continues the release, choosing `Cancel` stops jrit
before it changes anything.

## Preflight checks

`jrit doctor` verifies that the project is ready for a release and prints a report with one line per check. The same
checks run automatically after `jrit.toml` is parsed and before the release changes anything: if any of them fails, jrit
prints the whole report and stops. Warnings are informational and never stop the run. With `--dry-run` the report is
printed but the problems do not stop the plan.

The checks cover the release prerequisites:

- `jrit.toml` is found, parses and validates, every configured version file exists and its version field reads, and the
  changelog file exists when changelog handling is enabled
- the project is inside a git repository, `origin` is configured, and the current branch is in the allowed list and is
  not detached; uncommitted changes, tracked and untracked alike, are only a warning and never block the release
- the local branch is not behind `origin`, and local tags match `origin`; `jrit doctor` fetches tags first, a dry run
  only compares them and writes nothing
- GitHub credentials are available (`gh` CLI, `GITHUB_TOKEN`, `GH_TOKEN` or the jrit config) and can access the
  configured repository; this check is skipped in `ci` release mode

The git and GitHub checks talk to `origin` and the GitHub API, so they need network access.

After you pick the release version, jrit also verifies that the chosen tag does not already exist locally or on `origin`
and stops before changing anything if it does.

## Changelog

jrit expects a `CHANGELOG.md` in [Keep a Changelog](https://keepachangelog.com) format. Each release section must start
with `## [version]`:

```markdown
## [1.2.0] - 2024-05-01

### Features

- add dark mode support

### Bug Fixes

- fix crash on empty config
```

jrit extracts the section matching the selected version and uses it as the GitHub release body.

If the changelog section for the selected version is missing, jrit will abort and ask you to update it first.

### Generating a changelog draft

jrit can generate a changelog draft from your git history and open it in `$EDITOR` for review before saving. It parses
commits since the last tag using [Conventional Commits](https://www.conventionalcommits.org) and groups them by type.
After you close the editor, the result is prepended to `CHANGELOG.md` and the pipeline continues.

This step requires commits to follow the conventional commits format (`feat:`, `fix:`, `refactor:`, etc.). Commits
that don't match any known type are excluded from the draft.

## Configuration

jrit is configured via a `jrit.toml` file in the project root.

```toml
[project]
name = "my-app"
repo = "owner/repo"               # GitHub repo in owner/repo format
branches = ["main", "master"]     # allowed release branches; releasing from any other branch is blocked

changelog_type = "conventional"   # optional: "conventional" | "raw" | "manual" | "none" (default: "none")
changelog = "CHANGELOG.md"        # required if changelog_type is not "none"

release_mode = "local"            # optional: "local" | "ci" (default: "local")

sha256sums = true                 # optional: attach sha256sums.txt to the release (default: false)

[[components]]
name = "main"
path = "."                            # directory of the component, relative to jrit.toml
build = "cargo build --release"       # build command, run from the component's path (local mode only)
artifact = "./target/release/my-app"  # path to the built binary or directory, relative to component path (local mode only)
zip = true                            # pack artifact into a zip before uploading (default: true, local mode only)

[[components.version_files]]
file = "Cargo.toml"
path = ["package", "version"]         # key path to the version field; can be omitted for known files
```

### `changelog_type`

Controls how the changelog is handled:

- `none` (default) — changelog is ignored entirely. No draft generation, no parsing.
- `conventional` — generates a structured draft from commits
  following [Conventional Commits](https://www.conventionalcommits.org) format (`feat:`, `fix:`, `refactor:`, etc.),
  groups them by type, opens in `$EDITOR` for review, then prepends to the changelog file.
- `raw` — dumps raw `git log --oneline` since the last tag into a temp file and opens it in `$EDITOR`. You format it
  yourself.
- `manual` — skip changelog generation, only check that the expected changelog section exists in the file.

After you close the editor, the result is prepended to the changelog file and the pipeline continues.

### `release_mode`

Controls which steps are executed:

- `local` (default) — full pipeline: bump versions, build artifacts, commit, tag, publish GitHub release.
- `ci` — jrit only bumps versions, commits, and pushes a tag. Build and GitHub release are handled by your CI on tag
  trigger. Useful for cross-platform builds with GitHub Actions matrix. The `build`, `artifact`, and `zip` fields in
  `[[components]]` are ignored in this mode.

### `changelog` path

Required when `changelog_type` is not `"none"`. Path is relative to `jrit.toml`.

### Artifact upload behavior

The `zip` flag controls how artifacts are uploaded to the GitHub release:

- `zip = true` (default) — artifact is packed into a `.zip` and uploaded as a single file. Works for both files and
  directories.
- `zip = false` — artifact is uploaded as-is. If `artifact` points to a directory, each file inside is uploaded
  individually. Nested subdirectories are packed into separate zips automatically.

### `sha256sums`

Disabled by default and must be turned on explicitly with `sha256sums = true` in `[project]`. When enabled (local
release mode only), jrit generates a `sha256sums.txt` file with a SHA-256 checksum for every uploaded artifact and
attaches it to the GitHub release. When the file is not attached, `jrit update` skips checksum verification and prints a
warning.

### `path` in `version_files`

Key path inside the file pointing to the version string. Can be omitted for known file names — jrit will infer it
automatically:

| File            | Inferred path            |
|-----------------|--------------------------|
| `Cargo.toml`    | `["package", "version"]` |
| `package.json`  | `["version"]`            |
| `manifest.json` | `["version"]`            |

For any other file, `path` is required.

### Multiple version files per component

```toml
[[components]]
name = "obsidian-plugin"
path = "."
build = "npm run build"
artifact = "./dist"
zip = false

[[components.version_files]]
file = "manifest.json"

[[components.version_files]]
file = "package.json"
```

### Multiple components

```toml
[[components]]
name = "backend"
path = "backend"
build = "cargo build --release"
artifact = "./target/release/backend"

[[components.version_files]]
file = "Cargo.toml"

[[components]]
name = "frontend"
path = "frontend"
build = "npm run build"
artifact = "./dist"
zip = true

[[components.version_files]]
file = "package.json"
```

## Rollback behavior

If any step fails after versions have been bumped, jrit rolls back all modified version files to their original content
before exiting.