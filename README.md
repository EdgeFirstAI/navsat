# Maivin NavSat

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Build Status](https://github.com/EdgeFirstAI/navsat/actions/workflows/build.yml/badge.svg)](https://github.com/EdgeFirstAI/navsat/actions/workflows/build.yml)
[![Test Status](https://github.com/EdgeFirstAI/navsat/actions/workflows/test.yml/badge.svg)](https://github.com/EdgeFirstAI/navsat/actions/workflows/test.yml)

Navigation Satellite System (GPS/GNSS) driver for EdgeFirst Maivin platform.

## Overview

Maivin NavSat is a ROS 2 node that provides GPS/GNSS positioning data via GPSD for the EdgeFirst Maivin platform.

## Features

- GPSD integration for GPS/GNSS data
- Real-time positioning information
- Tracy profiling support
- Zenoh integration for distributed communication
- NavSatFix message publishing

## Requirements

- Rust 1.70 or later
- ROS 2 Humble or later
- GPSD daemon
- GPS/GNSS hardware receiver

## Building

```bash
cargo build --release
```

## Running

```bash
cargo run --release
```

## Testing

```bash
cargo test
```

## Documentation

For detailed documentation, visit [EdgeFirst Documentation](https://doc.edgefirst.ai/latest/maivin/).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on contributing to this project.

## License

Copyright 2025 Au-Zone Technologies Inc.

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

## Security

For security vulnerabilities, see [SECURITY.md](SECURITY.md).
