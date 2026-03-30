# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).





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
