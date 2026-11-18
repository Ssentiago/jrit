## [0.6.0] - 2026-11-18

### Features
- add release name template ([`988b359`](https://github.com/Ssentiago/jrit/commit/988b35959a50d480c24b49100a7b2d83de78637a))
- make sha256sums.txt upload opt-in ([`1f4a408`](https://github.com/Ssentiago/jrit/commit/1f4a408cf80058f1805f3515203047d0bc82f7b8))

## [0.5.0] - 2026-11-15

### Features
- add preflight checks before release ([`55c04af`](https://github.com/Ssentiago/jrit/commit/55c04afd22a9c37481c268ee61321dacc2624c19))
- add --dry-run release plan preview ([`17e00c5`](https://github.com/Ssentiago/jrit/commit/17e00c51b153826e4b945d88fc26643c676b912d))
- preview commits and changelog in version menu ([`2f28b6e`](https://github.com/Ssentiago/jrit/commit/2f28b6e3405fc8592bd88f15cd1b1fc1a521a1bb))
- verify asset checksums and read token from env ([`84c1437`](https://github.com/Ssentiago/jrit/commit/84c1437752edc4edc32f0a1d489bd8c53a611d08))

### Bug Fixes
- unify previous tag lookup across generation modes ([`77d7beb`](https://github.com/Ssentiago/jrit/commit/77d7beb231e9d2d11849dae9ea0a91066db57cd6))
- reject empty branch entries in validation ([`eea0d6b`](https://github.com/Ssentiago/jrit/commit/eea0d6b7f415ca15a0cff471a1f0e259bc0b6a12))
- report real changelog path in section-missing error ([`5a412fe`](https://github.com/Ssentiago/jrit/commit/5a412fe12b196b11565b8cff68cab1a4b0f4a660))
- run editors with args and trim empty-file prepend ([`25fe87a`](https://github.com/Ssentiago/jrit/commit/25fe87a245dc01e51ced7a93f55b33a031723c37))
- tolerate non-semver tags in version selection ([`cbe250e`](https://github.com/Ssentiago/jrit/commit/cbe250e78bf98f065961378becfbb98f99e4c963))
- delete release when asset upload fails ([`6f145cd`](https://github.com/Ssentiago/jrit/commit/6f145cdb06ae0e1800e39bb6aa544a119b5c7aab))
- keep zip archives out of scanned artifact directory ([`14a6faa`](https://github.com/Ssentiago/jrit/commit/14a6faa3449129b82296abb56bd7d9ed4efa65a6))
- check download status and accept v-prefixed tags ([`415d02e`](https://github.com/Ssentiago/jrit/commit/415d02e588206b45f54d13d5d0f6727835db63f1))
- abort cleanly on ctrl-c in prompts ([`c836179`](https://github.com/Ssentiago/jrit/commit/c83617910cbc097a0766b91d77eff27bcf72f330))
- keep TOML comments and formatting when bumping ([`616d163`](https://github.com/Ssentiago/jrit/commit/616d16366baebdcb236311145a3c7a8e5ef88db1))
- require strictly newer version in manual input ([`1d4697e`](https://github.com/Ssentiago/jrit/commit/1d4697efd5e2516affa355d373ced477c5588c90))
- serialize release runs and narrow build permissions ([`731d751`](https://github.com/Ssentiago/jrit/commit/731d751f5248c8d21cf79bdbac80e23c2d688ea2))
- use binary name for lipo sources and stage steps ([`db83857`](https://github.com/Ssentiago/jrit/commit/db83857f966f25c92ce00ac27838c3998feebb12))
- stage build output before artifact upload ([`e5076c5`](https://github.com/Ssentiago/jrit/commit/e5076c5962d2baf1b6fc9f7d54c1fb0826a0e399))
- match release tag literally when building body ([`80ae88c`](https://github.com/Ssentiago/jrit/commit/80ae88c75907f7dbcb510172e63a1d3c7323accd))
- fail release when artifacts are missing ([`88a6684`](https://github.com/Ssentiago/jrit/commit/88a66845c99bbcdcb83fdb2e381bcfa6a7c6f05b))
- return error on non-UTF8 paths instead of panicking ([`8899d28`](https://github.com/Ssentiago/jrit/commit/8899d2813060b6b72a5772dcdbea5aafb9ef4f46))
- preserve foreign index and uncommitted changes on rollback ([`863a523`](https://github.com/Ssentiago/jrit/commit/863a523604979b39d1a1a55fa7d1bf1a8167c5cb))
- skip commit when nothing is staged ([`2c30771`](https://github.com/Ssentiago/jrit/commit/2c307715fd0ef5b23ae0071514fb96de31bd9d47))
- settle branch before writing the changelog ([`f42dc0a`](https://github.com/Ssentiago/jrit/commit/f42dc0a0a341408b11b1218aa9dbb38420ca86c8))
- abort run when version selection is declined ([`213933b`](https://github.com/Ssentiago/jrit/commit/213933bfc720813414a7629cf7c86380661e3f93))
- resolve version and changelog paths from project root ([`f84074a`](https://github.com/Ssentiago/jrit/commit/f84074a6eb22444447ff9238d025bd2db1ef79fe))

### Refactoring
- split runner into typed steps and unify bumpers ([`ba2d3d2`](https://github.com/Ssentiago/jrit/commit/ba2d3d26a9f6a407df251dc7ff33f3dad360ed93))
- remove unused imports and redundant mut binding ([`f4528c7`](https://github.com/Ssentiago/jrit/commit/f4528c741cf35d24be5bbad8df25dc9195942467))

## [0.4.1] - 2026-07-23

### Bug Fixes
- skip changelog date in release body ([`16a81a8`](https://github.com/Ssentiago/jrit/commit/16a81a80aacba23fde0ad3ae1cf7407f8251d2da))

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
