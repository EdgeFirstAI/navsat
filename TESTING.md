# Testing Strategy for EdgeFirst NavSat

This document describes the comprehensive testing and coverage strategy for the EdgeFirst NavSat project, implemented according to **SPS 11-cicd-pipelines.md v2.4**.

## Overview

The EdgeFirst NavSat project uses a **three-phase testing pattern** with comprehensive GPS validation and end-to-end Zenoh integration testing across development, CI, and production hardware environments.

## Test Environments

### Phase 1: Development & CI Testing
- **Platform**: `ubuntu-22.04` (x86_64 and aarch64)
- **Tests**: Unit tests, integration tests (mocked)
- **Coverage**: LLVM coverage instrumentation
- **When**: On every push and pull request

### Phase 2: Hardware Testing (Raivin Platform)
- **Platform**: `raivin` self-hosted runner  
- **Hardware**: Real GPS receiver via GPSD
- **Tests**: 
  - Rust hardware integration tests (marked with `#[ignore]`)
  - Python Zenoh end-to-end integration tests
- **Coverage**: Runtime profraw collection
- **When**: On all pushes and pull requests

### Phase 3: Coverage Processing
- **Platform**: `ubuntu-22.04` (matching build toolchain)
- **Task**: Process profraw files from hardware tests
- **Output**: LCOV format for SonarCloud

## Running Tests Locally

### Unit Tests (No Hardware Required)
```bash
cargo test
```

Output:
```
test result: ok. 8 passed; 0 failed; 5 ignored
```

### All Rust Tests Including Hardware Tests
```bash
cargo test -- --include-ignored
```

**Note**: Hardware tests will fail without GPSD and GPS receiver. They are designed to run only on the `raivin` platform.

### Python Zenoh Integration Tests
```bash
# Install dependencies
pip install -r tests/requirements.txt

# Run pytest (requires edgefirst-navsat service running)
pytest tests/test_zenoh_integration.py -v
```

## Test Categories

### 1. Unit Tests (Always Run - 8 tests)
- `test_timestamp_returns_valid_time` - Validates timestamp generation
- `test_timestamp_is_monotonic` - Ensures time moves forward
- `test_create_navsat_fix_from_tpv_with_data` - TPV message creation
- `test_create_navsat_fix_from_tpv_with_none_values` - Null handling
- `test_create_navsat_fix_from_gst_with_data` - GST message creation
- `test_create_navsat_fix_from_gst_with_none_values` - Null handling  
- `test_navsat_fix_covariance_is_unknown` - Covariance defaults
- `test_create_navsat_fix_header_frame_id_is_empty` - Header validation

### 2. Rust Hardware Integration Tests (`#[ignore]` - 5 tests)

#### test_gpsd_connection
- **Purpose**: Validates GPSD daemon connectivity
- **What it checks**: TCP connection to GPSD on localhost:2947
- **Failure indicates**: GPSD not running or misconfigured

#### test_gps_fix_quality
- **Purpose**: Validates GPS receiver has achieved good 3D fix
- **What it checks**:
  - Fix mode is 3D (Mode::Fix3d)
  - At least 4 satellites used for 3D fix
  - Position data (lat/lon) is present
  - Altitude is reasonable
- **Metrics collected**:
  - Fix quality (No Fix / 2D / 3D)
  - Satellite count (used vs visible)
  - Position coordinates
  - Altitude
  - DOP values (HDOP, VDOP, PDOP) if available
- **Failure indicates**: Poor antenna placement, obstructed sky view, or GPS receiver issues

#### test_gps_signal_quality
- **Purpose**: Validates GPS signal strength is sufficient
- **What it checks**:
  - At least one satellite visible
  - Maximum SNR > 20 dB
  - Average SNR > 12 dB (realistic threshold for real-world conditions)
- **Metrics collected**:
  - SNR (Signal-to-Noise Ratio) for all satellites
  - Maximum and average SNR values
- **Failure indicates**: Weak signal, poor antenna, or interference

#### test_gps_position_reporting
- **Purpose**: Records actual GPS position for test verification
- **What it checks**:
  - Valid latitude (-90° to +90°)
  - Valid longitude (-180° to +180°)
  - Position is not default (0, 0)
- **Metrics collected**:
  - Test location coordinates
  - Altitude
  - Position range over test period
- **Output**: Prints test location for manual verification

#### test_hardware_timestamp_accuracy
- **Purpose**: Validates monotonic timestamp generation
- **What it checks**:
  - Timestamps are monotonically increasing (time moves forward)
  - Elapsed time measurements are accurate (~100ms test)
  - CLOCK_MONOTONIC_RAW is functioning correctly
- **Note**: Uses monotonic time (time since boot), not wall-clock time. This is correct for ROS message timing which requires monotonic timestamps for relative timing.
- **Failure indicates**: System clock issues or kernel timer problems

### 3. Python Zenoh Integration Tests (5 tests)

#### test_service_publishes_messages
- **Purpose**: Validates edgefirst-navsat service publishes to Zenoh
- **What it checks**:
  - At least 5 NavSatFix messages received within 60 seconds
  - Messages deserialize correctly using edgefirst-schemas
- **Failure indicates**: Service not running or Zenoh network issues

#### test_message_format_validity
- **Purpose**: Validates NavSatFix message format compliance
- **What it checks**:
  - Latitude in valid range (-90° to +90°)
  - Longitude in valid range (-180° to +180°)
  - Altitude is reasonable (< 10km)
  - Status fields present and valid
- **Failure indicates**: Message serialization bugs

#### test_gps_fix_quality
- **Purpose**: Validates GPS fix quality over Zenoh messages
- **What it checks**:
  - At least 50% of messages have fix quality >= 2 (2D or better)
- **Metrics collected**:
  - Fix quality distribution
  - Percentage of good fixes
- **Failure indicates**: GPS not achieving consistent fix

#### test_gps_position_reported
- **Purpose**: Validates position reporting and records test location
- **What it checks**:
  - Position is not default (0, 0)
- **Metrics collected**:
  - Average position over test period
  - Position range (min/max lat/lon/alt)
- **Output**: Prints test location for verification

#### test_message_timestamp_present
- **Purpose**: Validates message timestamps
- **What it checks**:
  - Header exists with timestamp
  - Timestamp is non-zero
- **Failure indicates**: Time synchronization or header generation issues

## Coverage Collection

### CI Coverage (x86_64 + aarch64)
```bash
cargo llvm-cov --workspace --lcov --output-path coverage.lcov
```

### Hardware Coverage (Raivin)
Coverage is collected automatically when hardware tests run:
1. Instrumented binaries built on ARM runner
2. Tests executed on Raivin with `LLVM_PROFILE_FILE` set
3. Profraw files collected and uploaded
4. Processing phase merges profraw with instrumented objects
5. Final LCOV report sent to SonarCloud

### Zenoh Integration Coverage
The edgefirst-navsat service is built with coverage enabled and profraw files collected during the Python test execution.

## CI/CD Workflows

### test.yml
- **Triggers**: Push to main/develop, PRs
- **Jobs**:
  - `format` - rustfmt check (nightly)
  - `clippy` - Linting
  - `build-and-test` - Build + unit tests on x86_64 and aarch64
  - `hardware-test` - Rust GPS integration tests on Raivin (conditional)
  - `zenoh-integration-test` - Python end-to-end tests on Raivin (conditional)
  - `process-hardware-coverage` - Merge coverage from hardware
  - `sonarcloud` - Static analysis and coverage reporting

### Hardware Test Conditions
Hardware tests run on all pushes to `main`/`develop` and all pull requests targeting those branches.

## Test Metrics and Reporting

All hardware tests print detailed metrics to aid in debugging and verification:

### GPS Hardware Test Metrics
```
=== GPS Hardware Test Metrics ===
Fix Quality: 3D Fix
Satellites: 8 used / 12 visible
Position: 45.421500°, -75.697200° @ 82.5m
DOP: HDOP=0.95 VDOP=1.20 PDOP=1.52
SNR: max=42.3 dB
     avg=35.1 dB
=================================
```

### Python Zenoh Test Metrics
```
=== GPS Position Metrics ===
Messages analyzed: 10
Position range:
  Latitude:  45.421450° to 45.421550°
  Longitude: -75.697150° to -75.697250°
  Altitude:  82.0m to 83.0m

Test location (average):
  45.421500°, -75.697200° @ 82.5m
============================
```

## GitHub Actions Security

All GitHub Actions use **hash-pinned versions** for security compliance:

```yaml
# ✅ CORRECT - Hash pinned
- uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683  # v4.2.2

# ❌ WRONG - Symbolic version (insecure)
- uses: actions/checkout@v4
```

This prevents supply chain attacks via malicious action updates.

## SonarCloud Integration

Coverage reports are submitted to SonarCloud in LCOV format:
- CI unit test coverage (x86_64 + aarch64)
- Hardware Rust integration test coverage (Raivin)
- Zenoh integration test coverage (Raivin)

View reports at: https://sonarcloud.io/project/overview?id=EdgeFirstAI_navsat

## Troubleshooting

### "No coverage from hardware tests"
**Cause**: Profraw files don't match instrumented binaries  
**Solution**: Ensure Phase 3 uses same toolchain as Phase 1 (both on `ubuntu-22.04`)

### "Hardware tests fail in CI"
**Expected**: Hardware tests are marked `#[ignore]` and skip on CI runners  
**Action**: They only run on `raivin` self-hosted runner with label conditions

### "GPSD connection failed"
**Cause**: GPSD daemon not running on hardware  
**Solution**: Check `systemctl status gpsd` on Raivin runner

### "GPS fix quality test fails"
**Cause**: GPS not achieving 3D fix  
**Solutions**:
- Check antenna placement (needs clear sky view)
- Verify antenna connection
- Allow more time for satellite acquisition (cold start can take 30+ seconds)
- Check for RF interference

### "Python Zenoh test fails"
**Cause**: edgefirst-navsat service not publishing
**Solutions**:
- Verify service started correctly
- Check Zenoh configuration (localhost interface)
- Verify GPSD is providing data to service
- Check logs from edgefirst-navsat service

## References

- [SPS 11-cicd-pipelines.md](~/Documents/SPS/11-cicd-pipelines.md) - CI/CD standards
- [SonarCloud Dashboard](https://sonarcloud.io/project/overview?id=EdgeFirstAI_navsat)
- [GitHub Actions Workflows](.github/workflows/)
- [EdgeFirst Samples - GPS Example](https://github.com/EdgeFirstAI/samples/blob/main/python/gps.py)

---

**Last Updated**: 2025-12-02  
**SPS Version**: 2.4
