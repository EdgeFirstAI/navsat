use zenoh_ros_type::common_interfaces::sensor_msgs::NavSatStatus;
use zenoh_ros_type::common_interfaces::sensor_msgs::NavSatFix;
use zenoh_ros_type::rcl_interfaces::builtin_interfaces::Time;
use zenoh_ros_type::common_interfaces::std_msgs::Header;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn header(frame_id: &str) -> Header {
    let time_now = SystemTime::now();
    let time_now = time_now
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
    Header {
        stamp: Time {
                sec: time_now.as_secs() as i32,
                nanosec: time_now.subsec_nanos() as u32,
            },
        frame_id: String::from(frame_id),
    }
}

pub fn gps_status(status: i8, service: u16) -> NavSatStatus {
    NavSatStatus {
        status,
        service,
    }
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

