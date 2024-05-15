use cdr::{CdrLe, Infinite};
use clap::Parser;
use edgefirst_schemas::{
    builtin_interfaces,
    sensor_msgs::{nav_sat_fix, nav_sat_status, NavSatFix, NavSatStatus},
    std_msgs::Header,
};
use gpsd_proto::{get_data, handshake, GpsdError, ResponseData};
use itertools::Itertools;
use log::{debug, info, warn};
use std::{
    io::{self, Error},
    net::TcpStream,
    str::FromStr,
};
use zenoh::prelude::sync::*;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// zenoh connection mode.
    #[arg(short = 'm', long = "mode", default_value = "client")]
    mode: String,

    /// connect to Zenoh endpoint.
    #[arg(short = 'c', long = "connect", default_value = "tcp/127.0.0.1:7447")]
    connect: Vec<String>,

    /// list to Zenoh endpoint.
    #[arg(short = 'l', long = "listen")]
    listen: Vec<String>,

    /// connect to GPS endpoint
    #[arg(short = 'g', long = "gps-endpoint", default_value = "127.0.0.1:2947")]
    gps_endpoint: String,

    /// ros topic.
    #[arg(short = 't', long = "topic", default_value = "rt/gps")]
    topic: String,
}

fn main() -> Result<(), GpsdError> {
    let args = Args::parse();
    env_logger::init();

    let mut config = Config::default();
    let mode = WhatAmI::from_str(&args.mode).unwrap();
    config.set_mode(Some(mode)).unwrap();
    config.connect.endpoints = args.connect.iter().map(|v| v.parse().unwrap()).collect();
    config.listen.endpoints = args.listen.iter().map(|v| v.parse().unwrap()).collect();
    let _ = config.scouting.multicast.set_enabled(Some(false));
    let _ = config.scouting.gossip.set_enabled(Some(false));
    let session = zenoh::open(config.clone()).res_sync().unwrap();

    let stream = TcpStream::connect(&args.gps_endpoint)?;
    let mut reader = io::BufReader::new(&stream);
    let mut writer = io::BufWriter::new(&stream);
    handshake(&mut reader, &mut writer)?;

    info!(
        "connected to gpsd {} publishing navsat messages on topic: {}",
        &args.gps_endpoint, &args.topic
    );

    loop {
        let msg = match get_data(&mut reader) {
            Ok(m) => m,
            Err(e) => {
                warn!("gpsd::get_data error: {}", e);
                continue;
            }
        };

        match msg {
            ResponseData::Device(d) => {
                debug!(
                    "DEVICE {} {} {}",
                    d.path.unwrap_or("".to_string()),
                    d.driver.unwrap_or("".to_string()),
                    d.activated.unwrap_or("".to_string())
                );
            }
            ResponseData::Tpv(t) => {
                debug!(
                    "{:3} {:8.5} {:8.5} {:6.1} m {:5.1} ° {:6.3} m/s",
                    t.mode.to_string(),
                    t.lat.unwrap_or(0.0),
                    t.lon.unwrap_or(0.0),
                    t.alt.unwrap_or(0.0),
                    t.track.unwrap_or(0.0),
                    t.speed.unwrap_or(0.0)
                );

                let msg = NavSatFix {
                    header: Header {
                        stamp: timestamp()?,
                        frame_id: "".to_owned(),
                    },
                    status: NavSatStatus {
                        status: nav_sat_status::STATUS_FIX,
                        service: nav_sat_status::SERVICE_GPS as u16,
                    },
                    latitude: t.lat.unwrap_or(0.0) as f64,
                    longitude: t.lon.unwrap_or(0.0) as f64,
                    altitude: t.alt.unwrap_or(0.0) as f64,
                    position_covariance: [-1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    position_covariance_type: nav_sat_fix::COVARIANCE_TYPE_UNKNOWN,
                };

                let encoded = cdr::serialize::<_, _, CdrLe>(&msg, Infinite).unwrap();
                session
                    .put(&args.topic, encoded)
                    .encoding(Encoding::WithSuffix(
                        KnownEncoding::AppOctetStream,
                        "sensor_msgs/msg/NavSatFix".into(),
                    ))
                    .res()
                    .unwrap();
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
                debug!(
                    "Sky xdop {:4.2} ydop {:4.2} vdop {:4.2}, satellites {}",
                    sky.xdop.unwrap_or(0.0),
                    sky.ydop.unwrap_or(0.0),
                    sky.vdop.unwrap_or(0.0),
                    sats
                );
            }
            ResponseData::Pps(p) => {
                debug!(
                    "PPS {} real: {} s {} ns clock: {} s {} ns precision: {}",
                    p.device, p.real_sec, p.real_nsec, p.clock_sec, p.clock_nsec, p.precision
                );
            }
            ResponseData::Gst(g) => {
                debug!(
                        "GST {} time: {} rms: {} major: {} m minor: {} m orient: {}° lat: {} m lon: {} m alt: {} m",
                        g.device.unwrap_or("".to_string()), g.time.unwrap_or("".to_string()),
                        g.rms.unwrap_or(0.), g.major.unwrap_or(0.),
                        g.minor.unwrap_or(0.), g.orient.unwrap_or(0.),
                        g.lat.unwrap_or(0.), g.lon.unwrap_or(0.), g.alt.unwrap_or(0.)
                    );

                let msg = NavSatFix {
                    header: Header {
                        stamp: timestamp()?,
                        frame_id: "".to_owned(),
                    },
                    status: NavSatStatus {
                        status: nav_sat_status::STATUS_FIX,
                        service: nav_sat_status::SERVICE_GPS as u16,
                    },
                    latitude: g.lat.unwrap_or(0.0) as f64,
                    longitude: g.lon.unwrap_or(0.0) as f64,
                    altitude: g.alt.unwrap_or(0.0) as f64,
                    position_covariance: [-1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    position_covariance_type: nav_sat_fix::COVARIANCE_TYPE_UNKNOWN,
                };

                let encoded = cdr::serialize::<_, _, CdrLe>(&msg, Infinite).unwrap();
                session
                    .put(&args.topic, encoded)
                    .encoding(Encoding::WithSuffix(
                        KnownEncoding::AppOctetStream,
                        "sensor_msgs/msg/NavSatFix".into(),
                    ))
                    .res()
                    .unwrap();
            }
        }
    }
}

fn timestamp() -> Result<builtin_interfaces::Time, Error> {
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
