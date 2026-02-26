# Changelog

All notable changes to EdgeFirst NavSat will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.5.0] - 2026-02-26

### Added

- Default configuration file (`navsat.default`) with documented settings for all options
- Release artifacts now include `navsat.default` configuration file

### Changed

- Use explicit environment variable names in CLI argument definitions
- Improved CLI argument help descriptions
- Updated clap from 4.5.53 to 4.5.60
- Updated edgefirst-schemas from 1.5.1 to 1.5.5
- Updated libc from 0.2.180 to 0.2.182

## [1.4.0] - 2026-01-30

### Added

- Graceful shutdown with SIGTERM/SIGINT signal handling for clean process termination
- Support for LLVM coverage profraw file flushing on shutdown

### Changed

- CI workflow uses nightly rustfmt for unstable formatting options
- Updated Makefile with `sbom` target for SPS compliance
- Upgraded edgefirst-schemas from 1.4.1 to 1.5.1
- Use `edgefirst_schemas::serde_cdr::serialize()` API instead of direct cdr crate
- Use `NavSatFix::SCHEMA_NAME` constant for schema type encoding
- Updated zenoh from 1.6.2 to 1.7.2
- Updated tracy-client from 0.18.3 to 0.18.4
- Updated libc from 0.2.177 to 0.2.180

### Removed

- Removed direct dependency on cdr crate (now using edgefirst-schemas re-export)

### Fixed

- Fixed NOTICE file format to match validation script requirements
- Fixed function cast warnings in signal handler installation

## [1.3.0] - 2025-11-29

### Added

- GitHub Actions CI/CD workflows (build, test, release, sbom)
- SPS v2.1.1 compliance documentation
- GitHub issue templates and PR template
- Security policy and vulnerability reporting process
- Unit tests for NavSatFix message creation and timestamp functions
- Library target (`src/lib.rs`) for improved testability

### Changed

- Migrated repository from Bitbucket to GitHub
- Updated license to Apache-2.0
- Updated dependencies (Zenoh 1.6.2, edgefirst-schemas 1.4.1)
- Refactored code into library and binary for better separation of concerns
- Improved error handling in message publishing (no longer panics on failure)

### Fixed

- Corrected binary name references in CI workflows
- Fixed test workflow to work with library target

## [1.2.0] - 2025-02-24

### Added

- Tracing instrumentation for performance monitoring
- Tracy profiler integration with broadcast support

### Changed

- Ported to Zenoh 1.2
- Updated Bitbucket Pipelines to use Rust 1.84.1
- Updated dependencies and automated code cleanups

## [1.1.0] - 2024-05-15

### Added

- Integration with edgefirst-schemas for ROS 2 message types

### Changed

- Use monotonic clock (CLOCK_MONOTONIC_RAW) for topic timestamps
- Disabled multicast scouting by default
- Code cleanup and Clippy fixes

### Fixed

- Improved error handling in GPSD data loop

## [1.0.2] - 2024-03-07

### Changed

- Zenoh client mode support
- Disabled multicast scouting by default

### Fixed

- Clippy formatting fixes

## [1.0.1] - 2024-03-05

### Added

- Bitbucket Pipelines for CI/CD
- Added encoding for message type

### Fixed

- Clippy code fixes

## [1.0.0] - 2023-11-10

### Added

- Initial release of Maivin NavSat service
- GPSD integration using gpsd_proto library
- NavSatFix message publishing via Zenoh
- Support for TPV (Time-Position-Velocity) data
- Support for GST (GPS Pseudorange Noise Statistics) data
- Configurable GPSD endpoint and ROS topic
- Verbose logging support

[Unreleased]: https://github.com/EdgeFirstAI/navsat/compare/v1.5.0...HEAD
[1.5.0]: https://github.com/EdgeFirstAI/navsat/compare/v1.4.0...v1.5.0
[1.4.0]: https://github.com/EdgeFirstAI/navsat/compare/v1.3.0...v1.4.0
[1.3.0]: https://github.com/EdgeFirstAI/navsat/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/EdgeFirstAI/navsat/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/EdgeFirstAI/navsat/compare/v1.0.2...v1.1.0
[1.0.2]: https://github.com/EdgeFirstAI/navsat/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/EdgeFirstAI/navsat/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/EdgeFirstAI/navsat/releases/tag/v1.0.0
