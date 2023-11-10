use std::time::Instant;
use zenoh_ros_type::{
    common_interfaces::{
        sensor_msgs::{NavSatFix, NavSatStatus},
        std_msgs::Header,
    },
    rcl_interfaces::builtin_interfaces::Time,
};

pub fn header(frame_id: &str, start_time: Instant) -> Header {
    Header {
        stamp: Time {
            sec: start_time.elapsed().as_secs() as i32,
            nanosec: start_time.elapsed().subsec_nanos() as u32,
        },
        frame_id: String::from(frame_id),
    }
}

pub fn gps_status(status: i8, service: u16) -> NavSatStatus {
    NavSatStatus { status, service }
}

pub fn gps_fix(
    header: Header,
    status: NavSatStatus,
    latitude: f64,
    longitude: f64,
    altitude: f64,
    position_covariance: [f64; 9],
    position_covariance_type: u8,
) -> NavSatFix {
    NavSatFix {
        header,
        status,
        latitude,
        longitude,
        altitude,
        position_covariance,
        position_covariance_type,
    }
}
