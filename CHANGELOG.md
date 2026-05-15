## [0.2.0] - 2026-05-15

### Features
- add `update` command to update app itself ([`51e2530`])
- add release job to generated workflow ([`557e4c9`])
- commit and push changelog when changelog_type is not none ([`14b813c`])
- add changelog_type `manual` ([`c1f15d5`])

### Bug Fixes
- correct jrit.toml generation ([`8760011`])
- update config validation ([`9b7b4d4`])
# Changelog

## [0.1.0] - 2026-05-14

### Features

- Initial release of jrit — a tool to automate releases
- Interactive version bump with git tag update
- Changelog generation from commits and parsing of existing changelog
- Config file support (`jrit.toml`) with `jrit init` command and optional CI workflow setup
- Full release pipeline: build → version bump → changelog → GitHub release upload
