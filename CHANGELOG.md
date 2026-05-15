# Changelog

## [0.1.0] - 2026-05-14

### Features

- Initial release of jrit — a tool to automate releases
- Interactive version bump with git tag update
- Changelog generation from commits and parsing of existing changelog
- Config file support (`jrit.toml`) with `jrit init` command and optional CI workflow setup
- Full release pipeline: build → version bump → changelog → GitHub release upload
