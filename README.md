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
jrit update    # update jrit itself to the latest version
```

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
branches = ["main", "master"]     # allowed release branches; if you're on a different branch, jrit will ask

changelog_type = "conventional"   # optional: "conventional" | "raw" | "manual" | "none" (default: "none")
changelog = "CHANGELOG.md"        # required if changelog_type is not "none"

release_mode = "local"            # optional: "local" | "ci" (default: "local")

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