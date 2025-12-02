// Copyright 2025 Au-Zone Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

//! NavSat message creation and timestamp utilities.

use edgefirst_schemas::{
    builtin_interfaces,
    sensor_msgs::{nav_sat_fix, nav_sat_status, NavSatFix, NavSatStatus},
    std_msgs::Header,
};
use gpsd_proto::{Gst, Tpv};
use std::io::Error;

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

/// Gets the current monotonic timestamp.
///
/// Uses `CLOCK_MONOTONIC_RAW` for consistent timing that is not affected
/// by NTP adjustments or system clock changes.
///
/// # Returns
///
/// A `Time` struct with seconds and nanoseconds, or an error if the
/// system call fails.
pub fn timestamp() -> Result<builtin_interfaces::Time, Error> {
    let mut tp = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let err = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC_RAW, &mut tp) };
    if err != 0 {
        return Err(Error::last_os_error());
    }

    Ok(builtin_interfaces::Time {
        sec: tp.tv_sec as i32,
        nanosec: tp.tv_nsec as u32,
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
    fn test_timestamp_returns_valid_time() {
        let time = timestamp().unwrap();
        assert!(time.sec >= 0);
        assert!(time.nanosec < 1_000_000_000);
    }

    #[test]
    fn test_timestamp_is_monotonic() {
        let time1 = timestamp().unwrap();
        let time2 = timestamp().unwrap();

        let nanos1 = (time1.sec as i64) * 1_000_000_000 + (time1.nanosec as i64);
        let nanos2 = (time2.sec as i64) * 1_000_000_000 + (time2.nanosec as i64);

        assert!(nanos2 >= nanos1);
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
}
