use cdr::{CdrLe, Infinite};
use clap::Parser;
use gpsd_proto::{get_data, handshake, GpsdError, ResponseData};
use itertools::Itertools;
use std::{
    io,
    net::TcpStream,
    thread::sleep,
    time::{Duration, Instant},
};
use zenoh::{prelude::sync::*, publication::CongestionControl};
use zenoh_ros_type::common_interfaces::sensor_msgs::{nav_sat_fix, nav_sat_status};

mod connection;
mod messages;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// zenoh connection mode.
    #[arg(short = 'm', long = "mode", default_value = "peer")]
    mode: String,

    /// connect to Zenoh endpoint.
    #[arg(short = 'e', long = "endpoint")]
    endpoint: Vec<String>,

    /// connect to GPS endpoint
    #[arg(short = 'g', long = "gps-endpoint", default_value = "127.0.0.1:2947")]
    gps_endpoint: String,

    /// ros topic.
    #[arg(short = 't', long = "topic", default_value = "rt/gps")]
    topic: String,

    /// Enable the verbose output.
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
}

fn main() -> Result<(), GpsdError> {
    let start_time = Instant::now();

    let args = Args::parse();

    // Start a Zenoh connection at the endpoint.
    let session = connection::start_session(&args.mode, &args.endpoint).unwrap();

    // Publish messages.
    macro_rules! log {
        ($( $args:expr ),*) => { if args.verbose {println!( $( $args ),* );} }
    }

    // Publish messages.
    let publisher = session
        .declare_publisher(args.topic.clone())
        .congestion_control(CongestionControl::Block)
        .res()
        .unwrap();

    if let Ok(stream) = TcpStream::connect(&args.gps_endpoint) {
        let mut reader = io::BufReader::new(&stream);
        let mut writer = io::BufWriter::new(&stream);
        handshake(&mut reader, &mut writer)?;

        let frame = String::from("GPSMap");
        println!("Publish GPS on '{}' for '{}')...", &args.topic, frame);

        loop {
            // Build the IMU message type.
            let header = messages::header(&frame, start_time);
            sleep(Duration::from_millis(50));

            //= get_data(&mut reader)?;
            let msg = match get_data(&mut reader) {
                Ok(m) => m,
                Err(e) => {
                    println!("{}", e);
                    continue;
                }
            };

            match msg {
                ResponseData::Device(d) => {
                    log!(
                        "DEVICE {} {} {}",
                        d.path.unwrap_or("".to_string()),
                        d.driver.unwrap_or("".to_string()),
                        d.activated.unwrap_or("".to_string())
                    );
                }
                ResponseData::Tpv(t) => {
                    log!(
                        "{:3} {:8.5} {:8.5} {:6.1} m {:5.1} ° {:6.3} m/s",
                        t.mode.to_string(),
                        t.lat.unwrap_or(0.0),
                        t.lon.unwrap_or(0.0),
                        t.alt.unwrap_or(0.0),
                        t.track.unwrap_or(0.0),
                        t.speed.unwrap_or(0.0)
                    );

                    let status = messages::gps_status(
                        nav_sat_status::STATUS_FIX,
                        nav_sat_status::SERVICE_GPS as u16,
                    ); // Service is unknown currently.
                    let nav_fix = messages::gps_fix(
                        header,
                        status,
                        t.lat.unwrap_or(0.0),
                        t.lon.unwrap_or(0.0),
                        t.alt.unwrap_or(0.0) as f64,
                        [-1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                        nav_sat_fix::COVARIANCE_TYPE_UNKNOWN,
                    );

                    let encoded = cdr::serialize::<_, _, CdrLe>(&nav_fix, Infinite).unwrap();
                    publisher.put(encoded).res().unwrap();
                }
                ResponseData::Sky(sky) => {
                    let sats = sky.satellites.map_or_else(
                        || "(none)".to_owned(),
                        |sats| {
                            sats.iter()
                                .filter(|sat| sat.used)
                                .map(|sat| sat.prn.to_string())
                                .join(",")
                        },
                    );
                    log!(
                        "Sky xdop {:4.2} ydop {:4.2} vdop {:4.2}, satellites {}",
                        sky.xdop.unwrap_or(0.0),
                        sky.ydop.unwrap_or(0.0),
                        sky.vdop.unwrap_or(0.0),
                        sats
                    );
                }
                ResponseData::Pps(p) => {
                    log!(
                        "PPS {} real: {} s {} ns clock: {} s {} ns precision: {}",
                        p.device,
                        p.real_sec,
                        p.real_nsec,
                        p.clock_sec,
                        p.clock_nsec,
                        p.precision
                    );
                }
                ResponseData::Gst(g) => {
                    log!(
                        "GST {} time: {} rms: {} major: {} m minor: {} m orient: {}° lat: {} m lon: {} m alt: {} m",
                        g.device.unwrap_or("".to_string()), g.time.unwrap_or("".to_string()),
                        g.rms.unwrap_or(0.), g.major.unwrap_or(0.),
                        g.minor.unwrap_or(0.), g.orient.unwrap_or(0.),
                        g.lat.unwrap_or(0.), g.lon.unwrap_or(0.), g.alt.unwrap_or(0.)
                    );

                    let status = messages::gps_status(
                        nav_sat_status::STATUS_FIX,
                        nav_sat_status::SERVICE_GPS as u16,
                    ); // Service is unknown currently.
                    let nav_fix = messages::gps_fix(
                        header,
                        status,
                        g.lat.unwrap_or(0.) as f64,
                        g.lon.unwrap_or(0.) as f64,
                        g.alt.unwrap_or(0.) as f64,
                        [-1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                        nav_sat_fix::COVARIANCE_TYPE_UNKNOWN,
                    );

                    let encoded = cdr::serialize::<_, _, CdrLe>(&nav_fix, Infinite).unwrap();
                    publisher.put(encoded).res().unwrap();
                }
            }
        }
    } else {
        panic!("Couldn't connect to gpsd...");
    }
}
