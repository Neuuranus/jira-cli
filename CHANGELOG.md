# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).














## [0.3.10](https://github.com/rvben/jira-cli/compare/v0.3.9...v0.3.10) - 2026-04-23

### Fixed

- **search**: harden v3 cursor walk and clean up search path ([3643291](https://github.com/rvben/jira-cli/commit/36432911a899cc9fe86efdd75e98a180a3d402cb))
- migrate Jira Cloud search to /rest/api/3/search/jql ([c858fa2](https://github.com/rvben/jira-cli/commit/c858fa251b437e274568680047adb584a8eacf34))

## [0.3.9](https://github.com/rvben/jira-cli/compare/v0.3.8...v0.3.9) - 2026-04-08

### Added

- publish to PyPI as jira-cli-rs ([8f5370a](https://github.com/rvben/jira-cli/commit/8f5370a26bd9b162be95c7f5e78a47a6771fd9a8))

## [0.3.8](https://github.com/rvben/jira-cli/compare/v0.3.7...v0.3.8) - 2026-04-07

### Added

- `jira issue PROJ-123` falls through to `issues show` ([3764b1f](https://github.com/rvben/jira-cli/commit/3764b1f746a60bd853677b7d38a32d014002c5fe))
- add singular aliases for all subcommand groups ([2d05eea](https://github.com/rvben/jira-cli/commit/2d05eea6fa74ae5ee1ddadf169a54afa43d1490d))

### Fixed

- schema tests acquire env lock to prevent XDG_CONFIG_HOME leakage ([204b794](https://github.com/rvben/jira-cli/commit/204b79422741328a81be0b70744a8a9078e8eb4b))

## [0.3.7](https://github.com/rvben/jira-cli/compare/v0.3.6...v0.3.7) - 2026-04-03

### Added

- add top-level `issue` command as shortcut for `issues show` ([788bcc4](https://github.com/rvben/jira-cli/commit/788bcc4722b5a23d1fb11d08fdfabf814e2c53f5))

## [0.3.6](https://github.com/rvben/jira-cli/compare/v0.3.5...v0.3.6) - 2026-04-03

## [0.3.5](https://github.com/rvben/jira-cli/compare/v0.3.4...v0.3.5) - 2026-04-03

## [0.3.4](https://github.com/rvben/jira-cli/compare/v0.3.3...v0.3.4) - 2026-04-01

### Added

- add read-only mode via JIRA_READ_ONLY env var and config field ([68e15a3](https://github.com/rvben/jira-cli/commit/68e15a353c5000488516ccc929597c6da5df7929))

## [0.3.3](https://github.com/rvben/jira-cli/compare/v0.3.2...v0.3.3) - 2026-03-31

### Fixed

- **config**: show token in plain text during init ([fd62572](https://github.com/rvben/jira-cli/commit/fd6257201119664fa280e0d3b8d30983450cac23))

## [0.3.2](https://github.com/rvben/jira-cli/compare/v0.3.1...v0.3.2) - 2026-03-31

### Added

- **config**: interactive init wizard and profile removal ([1db53db](https://github.com/rvben/jira-cli/commit/1db53dbaf75c65d0d0ae3fcde9de6e3b878ed8a8))

## [0.3.1](https://github.com/rvben/jira-cli/compare/v0.3.0...v0.3.1) - 2026-03-30

### Fixed

- simplify mount_board_and_sprints to async fn per clippy lint ([37e094b](https://github.com/rvben/jira-cli/commit/37e094b6fe2c1c6f8602c3faccaea1d8adcfbb73))

## [0.3.0](https://github.com/rvben/jira-cli/compare/v0.2.0...v0.3.0) - 2026-03-30

### Added

- **issues**: add worklog, bulk ops, and subtask support ([5383672](https://github.com/rvben/jira-cli/commit/53836728887f079934ed793a7be96665e9b152be))

## [0.2.0](https://github.com/rvben/jira-cli/compare/v0.1.0...v0.2.0) - 2026-03-30

### Added

- **issues**: add --all pagination, issues mine, and issues comments ([725def7](https://github.com/rvben/jira-cli/commit/725def78a7580e43a27473951ece76024050b82a))
- add users, boards, sprints, fields, issue links, and sprint assignment ([639fb26](https://github.com/rvben/jira-cli/commit/639fb2641a6ab744c66204f1b305c6e7b402b65d))
- improve config init output with DC/Server PAT instructions ([0193584](https://github.com/rvben/jira-cli/commit/01935847c8e02c50fba48864af9cc6edb554b2ce))
- add Jira Data Center / Server support ([f654ef3](https://github.com/rvben/jira-cli/commit/f654ef3c399f54b326ee0cdafe085caafd4b8327))

## [0.1.0](https://github.com/rvben/jira-cli/compare/v0.0.2...v0.1.0) - 2026-03-30

## [0.0.2] - 2026-03-30

### Added

- initial release of jira CLI ([e5f730b](https://github.com/rvben/jira-cli/commit/e5f730ba424a2b753d333fa389f0c3491d6f6402))

### Fixed

- align config bootstrap and schema contract ([a316125](https://github.com/rvben/jira-cli/commit/a316125cb243e209ecacf59af96980fbb4eace21))
- harden jira api behavior and pagination ([64956bf](https://github.com/rvben/jira-cli/commit/64956bfe702f094002d65cf476ddc01175283245))
