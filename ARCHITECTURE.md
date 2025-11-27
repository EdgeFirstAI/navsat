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

## Configuration

Configuration is managed through command-line arguments and environment variables. See `args.rs` for available options.

## Future Enhancements

- RTK (Real-Time Kinematic) support
- NTRIP client integration
- Enhanced satellite visibility reporting
- Multi-GPS receiver support
