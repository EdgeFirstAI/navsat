use zenoh_ros_type::common_interfaces::sensor_msgs::nav_sat_status;
use zenoh_ros_type::common_interfaces::sensor_msgs::nav_sat_fix;
use zenoh_ros_type::common_interfaces::sensor_msgs::NavSatFix;
use zenoh::prelude::r#async::*;
use cdr::{CdrLe, Infinite};
use async_std::task::sleep;
use std::time::Duration;
use std::io::{self};
use clap::Parser;

mod connection;
mod messages;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// zenoh connection mode.
    #[arg(short='m', long="mode", default_value = "peer")]
    mode: String,

    /// connect to endpoint.
    #[arg(short='e', long="endpoint")]
    endpoint: Vec<String>,

    /// ros topic.
    #[arg(short='t', long="topic", default_value = "rt/gps")]
    topic: String,

    /// publisher mode (default is subscriber mode).
    #[arg(short='p', long="publisher")]
    publisher: bool,
}

#[async_std::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();

    // Start a Zenoh connection at the endpoint.
    let session = connection::start_session(&args.mode, &args.endpoint).await.unwrap();
    
    // Publish messages.
    if args.publisher {
        let publisher = session.declare_publisher(&args.topic).res().await.unwrap();

        for _idx in 0..u32::MAX {
            
            sleep(Duration::from_millis(50)).await;
                    
            let frame = String::from("GPSMap");
            println!("Publish GPS on '{}' for '{}')...", &args.topic, frame);
            
            // Build the IMU message type.
            let header = messages:: header(&frame);
            let status = messages::gps_status(nav_sat_status::STATUS_NO_FIX, 1); // Service is unknown currently.
            let nav_fix = messages::gps_fix(
                header, 
                status, 
                51.05, 
                -114.07, 
                1045.0, 
                [-1.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0], 
                nav_sat_fix::COVARIANCE_TYPE_UNKNOWN
            );
            
            let encoded = cdr::serialize::<_, _, CdrLe>(&nav_fix, Infinite).unwrap();
            publisher.put(encoded).res().await.unwrap();
        }
        return Ok(());
    } else {
        let subscriber = session.declare_subscriber(&args.topic).res().await.unwrap();
        while let Ok(sample) = subscriber.recv_async().await {
            let decoded = cdr::deserialize_from::<_, NavSatFix, _>(sample.value.payload.reader(), Infinite).unwrap();
            println!("GPS latitude={}, longitude={}, altitude={}", decoded.latitude, decoded.longitude, decoded.altitude);
        }
        return Ok(());
    }
}
