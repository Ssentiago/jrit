## [0.4.0] - 2026-05-20

### Features
- resolve version files path from component path ([`92416c8`](https://github.com/Ssentiago/jrit/commit/92416c81ab0a693cbe97431e415e799963bc6336))
- add default version file options and a manual input ([`87c5949`](https://github.com/Ssentiago/jrit/commit/87c5949d29e5f8aba3873d3b2d7009d676d327d2))
- ensure component path is exists and this is a dir ([`91bb378`](https://github.com/Ssentiago/jrit/commit/91bb3789b847553319a5ecefcd7d62b8f4a25441))

## [0.3.1] - 2026-05-16

### Bug Fixes
- double blank line when prepending changelog to existing ([`1689a24`](https://github.com/Ssentiago/jrit/commit/1689a2406c012536798a6db9e298b2b10e45f395))
- correct repo and version args order ([`9a4d070`](https://github.com/Ssentiago/jrit/commit/9a4d070be444fac172eb870d09c3308ad071612c))

## [0.3.0] - 2026-05-16

### Features

- make commit hashes in release body clickable ([
  `bb8feac`](https://github.com/Ssentiago/jrit/commit/bb8feac3c56cbec03ad7f9513a5d5b8790496a00))

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
