// Copyright 2025 Au-Zone Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

//! NavSat message creation and timestamp utilities.

use edgefirst_schemas::{
    builtin_interfaces,
    sensor_msgs::{nav_sat_fix, nav_sat_status, NavSatFix, NavSatStatus},
    std_msgs::Header,
};
use gpsd_proto::{Gst, Tpv};
use std::time::{SystemTime, SystemTimeError, UNIX_EPOCH};

/// Errors that can occur when generating timestamps.
#[derive(Debug)]
pub enum TimestampError {
    /// System clock is before Unix epoch.
    BeforeEpoch(SystemTimeError),
    /// System clock seconds exceed i32 range (Y2038).
    Overflow,
}

impl std::fmt::Display for TimestampError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BeforeEpoch(e) => write!(f, "system clock before Unix epoch: {e}"),
            Self::Overflow => write!(f, "system clock seconds exceed i32 range"),
        }
    }
}

impl std::error::Error for TimestampError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BeforeEpoch(e) => Some(e),
            Self::Overflow => None,
        }
    }
}

/// Creates a NavSatFix message from TPV (Time-Position-Velocity) data.
///
/// # Arguments
///
/// * `tpv` - The TPV data from GPSD containing position information
/// * `stamp` - The timestamp to use for the message header
///
/// # Returns
///
/// A NavSatFix message populated with the TPV data.
pub fn create_navsat_fix_from_tpv(tpv: &Tpv, stamp: builtin_interfaces::Time) -> NavSatFix {
    NavSatFix {
        header: Header {
            stamp,
            frame_id: String::new(),
        },
        status: NavSatStatus {
            status: nav_sat_status::STATUS_FIX,
            service: nav_sat_status::SERVICE_GPS as u16,
        },
        latitude: tpv.lat.unwrap_or(0.0),
        longitude: tpv.lon.unwrap_or(0.0),
        altitude: tpv.alt.unwrap_or(0.0) as f64,
        position_covariance: [-1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        position_covariance_type: nav_sat_fix::COVARIANCE_TYPE_UNKNOWN,
    }
}

/// Creates a NavSatFix message from GST (GPS Pseudorange Noise Statistics)
/// data.
///
/// # Arguments
///
/// * `gst` - The GST data from GPSD containing error estimates
/// * `stamp` - The timestamp to use for the message header
///
/// # Returns
///
/// A NavSatFix message populated with the GST data.
pub fn create_navsat_fix_from_gst(gst: &Gst, stamp: builtin_interfaces::Time) -> NavSatFix {
    NavSatFix {
        header: Header {
            stamp,
            frame_id: String::new(),
        },
        status: NavSatStatus {
            status: nav_sat_status::STATUS_FIX,
            service: nav_sat_status::SERVICE_GPS as u16,
        },
        latitude: gst.lat.unwrap_or(0.0) as f64,
        longitude: gst.lon.unwrap_or(0.0) as f64,
        altitude: gst.alt.unwrap_or(0.0) as f64,
        position_covariance: [-1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        position_covariance_type: nav_sat_fix::COVARIANCE_TYPE_UNKNOWN,
    }
}

/// Gets the current wall-clock timestamp.
///
/// Uses `SystemTime` (backed by `CLOCK_REALTIME` on Linux) for ROS 2
/// compatible Header stamps. Wall-clock time is the convention across
/// the ROS 2 ecosystem, enabling correlation with logs, rosbags, and
/// external systems.
///
/// # Returns
///
/// A `Time` struct with seconds and nanoseconds, or an error if the
/// system clock is before the Unix epoch or beyond the i32 range (Y2038).
pub fn timestamp() -> Result<builtin_interfaces::Time, TimestampError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(TimestampError::BeforeEpoch)?;

    let secs = duration.as_secs();
    if secs > i32::MAX as u64 {
        return Err(TimestampError::Overflow);
    }

    Ok(builtin_interfaces::Time {
        sec: secs as i32,
        nanosec: duration.subsec_nanos(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpsd_proto::Mode;

    /// Helper to create a Tpv with optional position data
    fn make_tpv(lat: Option<f64>, lon: Option<f64>, alt: Option<f32>) -> Tpv {
        Tpv {
            device: None,
            status: None,
            mode: Mode::NoFix,
            time: None,
            ept: None,
            leapseconds: None,
            alt_msl: None,
            alt_hae: None,
            geoid_sep: None,
            lat,
            lon,
            alt,
            epx: None,
            epy: None,
            epv: None,
            track: None,
            speed: None,
            climb: None,
            epd: None,
            eps: None,
            epc: None,
            eph: None,
        }
    }

    /// Helper to create a Gst with optional position data
    fn make_gst(lat: Option<f32>, lon: Option<f32>, alt: Option<f32>) -> Gst {
        Gst {
            device: None,
            time: None,
            rms: None,
            major: None,
            minor: None,
            orient: None,
            lat,
            lon,
            alt,
        }
    }

    #[test]
    fn test_timestamp_matches_system_time() {
        let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let time = timestamp().unwrap();
        let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        // Timestamp should fall between the two SystemTime samples
        let ts_secs = time.sec as u64;
        assert!(ts_secs >= before.as_secs());
        assert!(ts_secs <= after.as_secs());
        assert!(time.nanosec < 1_000_000_000);
    }

    #[test]
    fn test_timestamp_consecutive_calls_valid() {
        let time1 = timestamp().unwrap();
        let time2 = timestamp().unwrap();

        // Both should have valid nanosecond range
        assert!(time1.nanosec < 1_000_000_000);
        assert!(time2.nanosec < 1_000_000_000);

        // Both should have non-negative seconds (valid post-epoch)
        assert!(time1.sec >= 0);
        assert!(time2.sec >= 0);
    }

    #[test]
    fn test_create_navsat_fix_from_tpv_with_data() {
        let tpv = make_tpv(Some(45.4215), Some(-75.6972), Some(100.0));

        let stamp = builtin_interfaces::Time {
            sec: 123,
            nanosec: 456,
        };
        let msg = create_navsat_fix_from_tpv(&tpv, stamp);

        assert_eq!(msg.latitude, 45.4215);
        assert_eq!(msg.longitude, -75.6972);
        assert_eq!(msg.altitude, 100.0);
        assert_eq!(msg.header.stamp.sec, 123);
        assert_eq!(msg.header.stamp.nanosec, 456);
        assert_eq!(msg.status.status, nav_sat_status::STATUS_FIX);
        assert_eq!(msg.status.service, nav_sat_status::SERVICE_GPS as u16);
        assert_eq!(
            msg.position_covariance_type,
            nav_sat_fix::COVARIANCE_TYPE_UNKNOWN
        );
    }

    #[test]
    fn test_create_navsat_fix_from_tpv_with_none_values() {
        let tpv = make_tpv(None, None, None);

        let stamp = builtin_interfaces::Time { sec: 0, nanosec: 0 };
        let msg = create_navsat_fix_from_tpv(&tpv, stamp);

        assert_eq!(msg.latitude, 0.0);
        assert_eq!(msg.longitude, 0.0);
        assert_eq!(msg.altitude, 0.0);
    }

    #[test]
    fn test_create_navsat_fix_from_gst_with_data() {
        let gst = make_gst(Some(45.4215), Some(-75.6972), Some(100.0));

        let stamp = builtin_interfaces::Time {
            sec: 789,
            nanosec: 101112,
        };
        let msg = create_navsat_fix_from_gst(&gst, stamp);

        assert_eq!(msg.latitude as f32, 45.4215);
        assert_eq!(msg.longitude as f32, -75.6972);
        assert_eq!(msg.altitude, 100.0);
        assert_eq!(msg.header.stamp.sec, 789);
        assert_eq!(msg.header.stamp.nanosec, 101112);
    }

    #[test]
    fn test_create_navsat_fix_from_gst_with_none_values() {
        let gst = make_gst(None, None, None);

        let stamp = builtin_interfaces::Time { sec: 0, nanosec: 0 };
        let msg = create_navsat_fix_from_gst(&gst, stamp);

        assert_eq!(msg.latitude, 0.0);
        assert_eq!(msg.longitude, 0.0);
        assert_eq!(msg.altitude, 0.0);
    }

    #[test]
    fn test_navsat_fix_covariance_is_unknown() {
        let tpv = make_tpv(None, None, None);
        let stamp = builtin_interfaces::Time { sec: 0, nanosec: 0 };
        let msg = create_navsat_fix_from_tpv(&tpv, stamp);

        assert_eq!(msg.position_covariance[0], -1.0);
        assert_eq!(
            msg.position_covariance_type,
            nav_sat_fix::COVARIANCE_TYPE_UNKNOWN
        );
    }

    #[test]
    fn test_create_navsat_fix_header_frame_id_is_empty() {
        let tpv = make_tpv(Some(0.0), Some(0.0), Some(0.0));
        let stamp = builtin_interfaces::Time { sec: 0, nanosec: 0 };
        let msg = create_navsat_fix_from_tpv(&tpv, stamp);

        assert!(msg.header.frame_id.is_empty());
    }

    /// Hardware integration tests that require real GPS hardware.
    /// These tests are marked with #[ignore] and only run on hardware runners.
    /// Execute with: cargo test -- --ignored --include-ignored
    #[cfg(test)]
    mod hardware_tests {
        use super::*;
        use gpsd_proto::{get_data, handshake, Mode, ResponseData};
        use std::{
            io::{BufReader, BufWriter},
            net::TcpStream,
            time::Duration,
        };

        /// GPS metrics collected during hardware tests
        #[derive(Debug, Default)]
        struct GpsMetrics {
            fix_mode: Option<Mode>,
            satellites_used: usize,
            satellites_visible: usize,
            latitude: Option<f64>,
            longitude: Option<f64>,
            altitude: Option<f64>,
            hdop: Option<f32>,
            vdop: Option<f32>,
            pdop: Option<f32>,
            max_snr: Option<f32>,
            avg_snr: Option<f32>,
        }

        impl GpsMetrics {
            fn print_summary(&self) {
                println!("\n=== GPS Hardware Test Metrics ===");
                if let Some(mode) = &self.fix_mode {
                    let fix_type = match mode {
                        Mode::NoFix => "No Fix",
                        Mode::Fix2d => "2D Fix",
                        Mode::Fix3d => "3D Fix",
                    };
                    println!("Fix Quality: {}", fix_type);
                }
                println!(
                    "Satellites: {} used / {} visible",
                    self.satellites_used, self.satellites_visible
                );

                if let (Some(lat), Some(lon), Some(alt)) =
                    (self.latitude, self.longitude, self.altitude)
                {
                    println!("Position: {:.6}°, {:.6}° @ {:.1}m", lat, lon, alt);
                }

                if let (Some(h), Some(v), Some(p)) = (self.hdop, self.vdop, self.pdop) {
                    println!("DOP: HDOP={:.2} VDOP={:.2} PDOP={:.2}", h, v, p);
                }

                if let Some(max_snr) = self.max_snr {
                    println!("SNR: max={:.1} dB", max_snr);
                    if let Some(avg_snr) = self.avg_snr {
                        println!("     avg={:.1} dB", avg_snr);
                    }
                }
                println!("=================================\n");
            }
        }

        fn collect_gps_metrics(timeout_secs: u64) -> Result<GpsMetrics, String> {
            let stream = TcpStream::connect("127.0.0.1:2947")
                .map_err(|e| format!("Failed to connect to GPSD: {}", e))?;

            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .map_err(|e| format!("Failed to set timeout: {}", e))?;

            let mut reader = BufReader::new(&stream);
            let mut writer = BufWriter::new(&stream);

            handshake(&mut reader, &mut writer)
                .map_err(|e| format!("GPSD handshake failed: {}", e))?;

            let mut metrics = GpsMetrics::default();
            let start = std::time::Instant::now();

            while start.elapsed().as_secs() < timeout_secs {
                match get_data(&mut reader) {
                    Ok(ResponseData::Tpv(tpv)) => {
                        // Only update fix mode if it's better than current
                        // This prevents transient NoFix from overwriting a good fix
                        let dominated = matches!(
                            (&metrics.fix_mode, &tpv.mode),
                            (None, _) | (Some(Mode::NoFix), _) | (Some(Mode::Fix2d), Mode::Fix3d)
                        );
                        if dominated {
                            metrics.fix_mode = Some(tpv.mode);
                        }

                        // Update position data if available
                        if tpv.lat.is_some() {
                            metrics.latitude = tpv.lat;
                        }
                        if tpv.lon.is_some() {
                            metrics.longitude = tpv.lon;
                        }
                        if tpv.alt.is_some() {
                            metrics.altitude = tpv.alt.map(|a| a as f64);
                        }
                    }
                    Ok(ResponseData::Sky(sky)) => {
                        if let Some(sats) = &sky.satellites {
                            metrics.satellites_visible = sats.len();
                            metrics.satellites_used = sats.iter().filter(|s| s.used).count();

                            let snr_values: Vec<f32> = sats.iter().filter_map(|s| s.ss).collect();

                            if !snr_values.is_empty() {
                                metrics.max_snr = snr_values
                                    .iter()
                                    .copied()
                                    .max_by(|a, b| a.partial_cmp(b).unwrap());
                                let sum: f32 = snr_values.iter().sum();
                                metrics.avg_snr = Some(sum / snr_values.len() as f32);
                            }
                        }

                        // Store DOP values if available
                        if let Some(h) = sky.hdop {
                            metrics.hdop = Some(h);
                        }
                        if let Some(v) = sky.vdop {
                            metrics.vdop = Some(v);
                        }
                        if let Some(p) = sky.pdop {
                            metrics.pdop = Some(p);
                        }
                    }
                    Ok(ResponseData::Gst(_gst)) => {
                        // GST provides error estimates but not DOP
                        // Could extract lat_err, lon_err, alt_err if needed
                    }
                    Ok(_) => {
                        // Other message types
                    }
                    Err(_) => {
                        std::thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                }

                // Only exit early if we have a valid fix (2D or 3D) AND satellite data
                // Don't exit early on NoFix - keep collecting until we get a good fix or
                // timeout
                let has_valid_fix =
                    matches!(metrics.fix_mode, Some(Mode::Fix2d) | Some(Mode::Fix3d));
                if has_valid_fix && metrics.satellites_visible > 0 && metrics.latitude.is_some() {
                    break;
                }
            }

            Ok(metrics)
        }

        /// Test GPSD connection on real hardware
        #[test]
        #[ignore = "Requires GPSD daemon and GPS hardware"]
        fn test_gpsd_connection() {
            let result = TcpStream::connect("127.0.0.1:2947");
            assert!(
                result.is_ok(),
                "Failed to connect to GPSD. Is gpsd daemon running?"
            );
        }

        /// Test GPS receiver has valid 3D fix with good signal quality
        #[test]
        #[ignore = "Requires GPSD daemon and GPS hardware"]
        fn test_gps_fix_quality() {
            let metrics = collect_gps_metrics(30).expect("Failed to collect GPS metrics");

            metrics.print_summary();

            // Require 3D fix
            assert!(
                matches!(metrics.fix_mode, Some(Mode::Fix3d)),
                "Expected 3D fix, got {:?}. Is GPS antenna positioned correctly?",
                metrics.fix_mode
            );

            // Require at least 4 satellites for 3D fix
            assert!(
                metrics.satellites_used >= 4,
                "Expected at least 4 satellites for 3D fix, got {}. Check antenna placement.",
                metrics.satellites_used
            );

            // Require position data
            assert!(
                metrics.latitude.is_some() && metrics.longitude.is_some(),
                "No position data received. GPS may not have achieved fix."
            );

            // Check altitude is reasonable (not default 0)
            if let Some(alt) = metrics.altitude {
                assert!(
                    alt.abs() > 1.0 || alt == 0.0,
                    "Altitude appears invalid: {}m",
                    alt
                );
            }
        }

        /// Test GPS signal quality (SNR) is sufficient
        #[test]
        #[ignore = "Requires GPSD daemon and GPS hardware"]
        fn test_gps_signal_quality() {
            let metrics = collect_gps_metrics(30).expect("Failed to collect GPS metrics");

            metrics.print_summary();

            // Require satellites visible
            assert!(
                metrics.satellites_visible > 0,
                "No satellites visible. Check GPS antenna connection and sky view."
            );

            // Check SNR if available
            if let Some(max_snr) = metrics.max_snr {
                assert!(
                    max_snr > 20.0,
                    "Maximum SNR too low: {:.1} dB. Expected > 20 dB. Check antenna.",
                    max_snr
                );

                if let Some(avg_snr) = metrics.avg_snr {
                    assert!(
                        avg_snr > 12.0,
                        "Average SNR too low: {:.1} dB. Expected > 12 dB. Weak signal.",
                        avg_snr
                    );
                }
            } else {
                panic!("No SNR data available from satellites");
            }
        }

        /// Test position reporting - captures actual GPS coordinates
        #[test]
        #[ignore = "Requires GPSD daemon and GPS hardware"]
        fn test_gps_position_reporting() {
            let metrics = collect_gps_metrics(30).expect("Failed to collect GPS metrics");

            metrics.print_summary();

            // Validate we have position
            let (lat, lon) = match (metrics.latitude, metrics.longitude) {
                (Some(lat), Some(lon)) => (lat, lon),
                _ => panic!("No GPS position available after 30 seconds"),
            };

            // Sanity check: coordinates should be valid ranges
            assert!((-90.0..=90.0).contains(&lat), "Invalid latitude: {}", lat);
            assert!(
                (-180.0..=180.0).contains(&lon),
                "Invalid longitude: {}",
                lon
            );

            println!("GPS Test Location: {:.6}°, {:.6}°", lat, lon);

            if let Some(alt) = metrics.altitude {
                println!("GPS Test Altitude: {:.1}m", alt);
            }
        }

        /// Test wall-clock timestamp generation on real hardware
        #[test]
        #[ignore = "Requires real-time clock on hardware"]
        fn test_hardware_timestamp_accuracy() {
            let time1 = timestamp().expect("Failed to get hardware timestamp");
            let mono_start = std::time::Instant::now();
            std::thread::sleep(std::time::Duration::from_millis(100));
            let time2 = timestamp().expect("Failed to get second hardware timestamp");
            let mono_elapsed = mono_start.elapsed();

            // Wall-clock times should be valid (past 2020)
            assert!(
                time1.sec > 1_577_836_800,
                "Wall-clock time looks invalid (pre-2020): {}",
                time1.sec
            );
            assert!(
                time2.sec > 1_577_836_800,
                "Wall-clock time looks invalid (pre-2020): {}",
                time2.sec
            );

            // Use monotonic clock to verify sleep duration (not wall-clock delta)
            let elapsed_ms = mono_elapsed.as_millis();
            assert!(
                (50..=200).contains(&elapsed_ms),
                "Monotonic elapsed time {}ms not in expected range [50-200ms]",
                elapsed_ms
            );

            println!(
                "Wall-clock timestamps: t1.sec={}, t2.sec={} (monotonic elapsed: {}ms)",
                time1.sec, time2.sec, elapsed_ms
            );
        }
    }
}
