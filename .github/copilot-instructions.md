# AI Assistant Instructions

This file provides guidance to AI coding assistants when working with code in this repository.

## Project Overview

EdgeFirst NavSat (`edgefirst-navsat`) is a GPS/GNSS driver for the EdgeFirst Maivin platform. It connects to GPSD, reads GPS positioning data, and publishes ROS 2 `NavSatFix` messages over Zenoh. It is a single-binary Rust application (not a ROS 2 node built with colcon).

## Build & Development Commands

```bash
# Format and lint (always run these before committing)
cargo fmt                          # Format code (uses rustfmt.toml settings)
cargo clippy --all-targets --all-features -- -D warnings  # Lint with strict warnings

# Build
cargo build                        # Debug build (native)
cargo build --release              # Release build (LTO + stripped)
cargo build --profile profiling    # Release with debug symbols for Tracy

# Test
cargo test                         # Unit tests only (8 tests)
cargo test -- --include-ignored    # All tests including hardware (requires GPSD + GPS receiver)
cargo test <test_name>             # Run a single test

# Makefile shortcuts
make format                        # Format with nightly rustfmt, falls back to stable
make lint                          # clippy --all-targets --all-features -- -D warnings
make test                          # Tests with coverage via cargo-nextest + cargo-llvm-cov
make pre-release                   # Full validation: format, lint, version check, test, SBOM
```

Required tools for full CI workflow: `cargo-nextest`, `cargo-llvm-cov`.

### Building for Linux

This project targets Linux (the Maivin embedded platform). When building on a non-Linux host (macOS, Windows) or cross-compiling for a different Linux architecture, use `cargo-zigbuild` instead of `cargo build`:

```bash
# Cross-compile for aarch64 Linux (Maivin target) from any host
cargo zigbuild --target aarch64-unknown-linux-gnu --release

# Cross-compile for x86_64 Linux from macOS
cargo zigbuild --target x86_64-unknown-linux-gnu --release
```

Do **not** use a `.cargo/config.toml` to configure cross-compilation toolchains — `cargo-zigbuild` handles the sysroot and linker automatically via Zig.

## Architecture

The binary has four source files in `src/`:

- **`main.rs`** - Entry point. Sets up signal handlers (SIGTERM/SIGINT for graceful shutdown and coverage flush), parses CLI args, initializes tracing (stdout + journald + Tracy), opens Zenoh session, connects to GPSD, and runs the main loop dispatching GPSD messages (TPV, GST, Sky, PPS, Device).
- **`args.rs`** - CLI argument parsing via `clap` with `derive`. All args have corresponding env vars (short names: `GPSD`, `TOPIC`, `MODE`, `CONNECT`, `LISTEN`, `TRACY`, `RUST_LOG`, `NO_MULTICAST_SCOUTING`). Implements `From<Args> for zenoh::Config` to convert args into Zenoh configuration.
- **`navsat.rs`** - Pure functions: `create_navsat_fix_from_tpv()`, `create_navsat_fix_from_gst()`, and `timestamp()` (uses `CLOCK_REALTIME` for ROS 2 compatible wall-clock stamps). All unit tests live here.
- **`lib.rs`** - Re-exports from `args` and `navsat` modules.

Data flow: `GPS Receiver -> GPSD daemon -> TCP -> navsat (parse + convert) -> Zenoh (CDR-encoded NavSatFix)`

Messages are serialized using CDR encoding via `edgefirst-schemas` and published with `APPLICATION_CDR` encoding + schema name.

## Key Dependencies

- **`edgefirst-schemas`** - ROS 2 message types (NavSatFix, Header, etc.) and CDR serialization
- **`gpsd_proto`** - GPSD protocol parsing (TPV, GST, Sky, PPS, Device messages)
- **`zenoh`** - Pub/sub transport (replaces ROS 2 DDS)
- **`tracing-tracy`** / **`tracy-client`** - Optional Tracy profiling (enabled by default via `tracy` feature, activated at runtime with `--tracy` flag)

## Cargo Features

- `default = ["tracy"]` - Tracy client compiled in but only active when `--tracy` flag is passed
- `profiling` - Adds sampling and system-tracing support for Tracy

## Testing

- **Unit tests** (8): Always run, no hardware needed. Test timestamp generation and NavSatFix message creation from TPV/GST data.
- **Hardware tests** (5, `#[ignore]`): Require GPSD + GPS receiver. Run on `raivin` self-hosted runner. Test GPSD connection, fix quality, signal quality (SNR), position reporting, and timestamp accuracy.
- **Python Zenoh integration tests** (`tests/`): End-to-end tests requiring the navsat service running. Use `pytest` with `tests/requirements.txt`.

Hardware tests run in CI only on `main` branch or PRs labeled `test-hardware`.

## Configuration

Runtime configuration is via CLI flags or environment variables. See `navsat.default` for the complete reference with all defaults and documentation. This file is loaded as a systemd `EnvironmentFile` in production.

## Code Style

- Always run `cargo fmt` and `cargo clippy --all-targets --all-features -- -D warnings` before committing. Both are enforced in CI.
- `rustfmt.toml`: `imports_granularity = 'Crate'`, `reorder_impl_items = true`, `use_field_init_shorthand = true`, `wrap_comments = true`, max line length 100
- SPDX headers required on all source files: `// Copyright 2025 Au-Zone Technologies Inc.` + `// SPDX-License-Identifier: Apache-2.0`
- Commits must be signed with DCO (`git commit -s`)
- Branch naming: `feature/<desc>`, `bugfix/<desc>`, `docs/<desc>`

## Clock Conventions

Header stamps use `CLOCK_REALTIME` (wall clock) per ROS 2 convention. This ensures timestamps are correlatable with logs, rosbags, and external systems. Use `CLOCK_MONOTONIC` only for internal duration/interval measurements.

