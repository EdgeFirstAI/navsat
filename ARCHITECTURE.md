# Architecture

## Overview

Maivin NavSat is a ROS 2 node that provides GPS/GNSS positioning data via GPSD for the EdgeFirst Maivin platform.

## System Architecture

### ROS 2 Node

The NavSat node operates as a ROS 2 component with the following responsibilities:

- Connect to GPSD daemon
- Read GPS/GNSS positioning data
- Publish NavSatFix messages to ROS 2 topics
- Provide fix quality and status information

### Key Components

1. **GPSD Client**
   - Connection management to GPSD daemon
   - Protocol handling
   - Data parsing

2. **Data Processing**
   - Coordinate conversion
   - Fix quality assessment
   - Status monitoring

3. **Output Generation**
   - ROS 2 NavSatFix messages
   - Position covariance
   - Fix status

## Communication

### Zenoh Integration

The NavSat node uses Zenoh for distributed communication, enabling:

- Low-latency data distribution
- Efficient network utilization
- Zero-copy transfers where applicable

### Data Flow

```
GPS Receiver → GPSD → NavSat Node → ROS 2 Topics
      ↓          ↓         ↓             ↓
  Hardware    Daemon  Processing    NavSatFix
  Interface           + Parsing     + Status
```

## Performance

### Tracy Profiling

The NavSat node includes Tracy profiling support for:

- Real-time performance monitoring
- Timing analysis
- Bottleneck identification

### GPS Capabilities

- Multi-constellation support (GPS, GLONASS, Galileo, BeiDou)
- Real-time positioning
- Fix quality reporting
- Velocity and heading information

## ROS 2 Year 2038 Limit

The ROS 2 `builtin_interfaces/msg/Time` message uses `int32` for the `sec` field, which overflows on 2038-01-19T03:14:07Z. This is an inherent limitation of the ROS 2 message definition.

NavSat handles this as follows:

1. The `timestamp()` function detects when `SystemTime` seconds exceed `i32::MAX` and returns a `TimestampError::Overflow` error.
2. The caller logs a warning and publishes the message with a saturated timestamp (`sec = i32::MAX`, `nanosec = 999_999_999`).
3. GPS data (position, fix quality, covariance) is still published — only the header timestamp is clamped.

This ensures the service continues delivering positioning data past 2038 rather than silently dropping messages. Downstream consumers should be aware that saturated timestamps indicate the Y2038 limit has been reached.

## Configuration

Configuration is managed through command-line arguments and environment variables. See `args.rs` for available options.

## Future Enhancements

- RTK (Real-Time Kinematic) support
- NTRIP client integration
- Enhanced satellite visibility reporting
- Multi-GPS receiver support
