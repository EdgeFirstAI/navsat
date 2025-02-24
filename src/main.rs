mod args;

use args::Args;
use cdr::{CdrLe, Infinite};
use clap::Parser;
use edgefirst_schemas::{
    builtin_interfaces,
    sensor_msgs::{nav_sat_fix, nav_sat_status, NavSatFix, NavSatStatus},
    std_msgs::Header,
};
use gpsd_proto::{get_data, handshake, GpsdError, ResponseData, Tpv};
use log::{debug, info, warn};
use std::{
    io::{self, Error},
    net::TcpStream,
};
use tracing::instrument;
use tracing_subscriber::{layer::SubscriberExt as _, Layer as _, Registry};
use tracy_client::{frame_mark, secondary_frame_mark};
use zenoh::{
    bytes::{Encoding, ZBytes},
    Session, Wait,
};

fn main() -> Result<(), GpsdError> {
    let args = Args::parse();

    args.tracy.then(tracy_client::Client::start);

    let stdout_log = tracing_subscriber::fmt::layer()
        .pretty()
        .with_filter(args.rust_log);

    let journald = match tracing_journald::layer() {
        Ok(journald) => Some(journald.with_filter(args.rust_log)),
        Err(_) => None,
    };

    let tracy = match args.tracy {
        true => Some(tracing_tracy::TracyLayer::default().with_filter(args.rust_log)),
        false => None,
    };

    let subscriber = Registry::default()
        .with(stdout_log)
        .with(journald)
        .with(tracy);
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
    tracing_log::LogTracer::init().unwrap();

    let session = zenoh::open(args.clone()).wait().unwrap();

    let stream = TcpStream::connect(&args.gpsd)?;
    let mut reader = io::BufReader::new(&stream);
    let mut writer = io::BufWriter::new(&stream);
    handshake(&mut reader, &mut writer)?;

    info!(
        "connected to gpsd {} publishing navsat messages on topic: {}",
        &args.gpsd, &args.topic
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
            ResponseData::Device(device) => handle_device(device),
            ResponseData::Tpv(tpv) => {
                handle_tpv(session.clone(), args.topic.clone(), tpv);
                args.tracy.then(|| secondary_frame_mark!("tpv"));
            }
            ResponseData::Sky(sky) => handle_sky(sky),
            ResponseData::Pps(pps) => handle_pps(pps),
            ResponseData::Gst(gst) => {
                handle_gst(session.clone(), args.topic.clone(), gst);
                args.tracy.then(|| secondary_frame_mark!("gst"));
            }
        }

        args.tracy.then(frame_mark);
    }
}

fn handle_device(device: gpsd_proto::Device) {
    debug!("{:?}", device);
}

fn handle_sky(sky: gpsd_proto::Sky) {
    debug!("{:?}", sky);
}

fn handle_pps(pps: gpsd_proto::Pps) {
    debug!("{:?}", pps);
}

#[instrument(skip_all)]
fn handle_tpv(session: Session, topic: String, tpv: Tpv) {
    debug!("{:?}", tpv);

    let msg = NavSatFix {
        header: Header {
            stamp: timestamp().unwrap(),
            frame_id: "".to_owned(),
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
    };

    let msg = ZBytes::from(cdr::serialize::<_, _, CdrLe>(&msg, Infinite).unwrap());
    let enc = Encoding::APPLICATION_CDR.with_schema("sensor_msgs/msg/NavSatFix");

    session.put(topic, msg).encoding(enc).wait().unwrap();
}

#[instrument(skip_all)]
fn handle_gst(session: Session, topic: String, gst: gpsd_proto::Gst) {
    debug!("{:?}", gst);

    let msg = NavSatFix {
        header: Header {
            stamp: timestamp().unwrap(),
            frame_id: "".to_owned(),
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
    };

    let msg = ZBytes::from(cdr::serialize::<_, _, CdrLe>(&msg, Infinite).unwrap());
    let enc = Encoding::APPLICATION_CDR.with_schema("sensor_msgs/msg/NavSatFix");

    session.put(topic, msg).encoding(enc).wait().unwrap();
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
