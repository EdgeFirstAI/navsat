# Changelog

All notable changes to Maivin NavSat will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- GitHub Actions CI/CD workflows (build, test, release, sbom)
- SPS v2.1.1 compliance documentation
- GitHub issue templates and PR template
- Security policy and vulnerability reporting process
- Unit tests for NavSatFix message creation and timestamp functions
- Library target (`src/lib.rs`) for improved testability
- Hardware test job using `raivin` self-hosted runner

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
- Zenoh client mode support
- Bitbucket Pipelines for CI/CD

### Changed

- Use monotonic clock (CLOCK_MONOTONIC_RAW) for topic timestamps
- Disabled multicast scouting by default
- Code cleanup and Clippy fixes

### Fixed

- Improved error handling in GPSD data loop

## [1.0.0] - 2023-11-10

### Added

- Initial release of Maivin NavSat service
- GPSD integration using gpsd_proto library
- NavSatFix message publishing via Zenoh
- Support for TPV (Time-Position-Velocity) data
- Support for GST (GPS Pseudorange Noise Statistics) data
- Configurable GPSD endpoint and ROS topic
- Verbose logging support

[Unreleased]: https://github.com/EdgeFirstAI/navsat/compare/v1.2.0...HEAD
[1.2.0]: https://github.com/EdgeFirstAI/navsat/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/EdgeFirstAI/navsat/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/EdgeFirstAI/navsat/releases/tag/v1.0.0
