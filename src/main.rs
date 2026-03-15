// Copyright 2025 Au-Zone Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use edgefirst_navsat::{
    create_navsat_fix_from_gst, create_navsat_fix_from_tpv, timestamp, Args, TimestampError,
};
use edgefirst_schemas::{
    schema_registry::SchemaType, sensor_msgs::NavSatFix, serde_cdr::serialize,
};
use gpsd_proto::{get_data, handshake, GpsdError, ResponseData};
use log::{debug, info, warn};
use std::{
    io::{self},
    net::TcpStream,
    sync::atomic::{AtomicBool, Ordering},
};
use tracing::instrument;
use tracing_subscriber::{layer::SubscriberExt as _, Layer as _, Registry};
use tracy_client::{frame_mark, secondary_frame_mark};
use zenoh::{
    bytes::{Encoding, ZBytes},
    Session, Wait,
};

/// Global shutdown flag for signal handling
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// Install signal handlers for graceful shutdown
/// This is critical for LLVM coverage instrumentation - without graceful
/// shutdown, profraw files are not flushed when the process receives SIGTERM.
fn install_signal_handlers() {
    unsafe {
        // Cast function pointer via *const () to avoid direct function-to-integer cast
        // warning
        let handler = handle_signal as *const () as libc::sighandler_t;
        // Handle SIGTERM (sent by kill command, systemd, etc.)
        libc::signal(libc::SIGTERM, handler);
        // Handle SIGINT (Ctrl+C)
        libc::signal(libc::SIGINT, handler);
    }
}

extern "C" fn handle_signal(_: libc::c_int) {
    SHUTDOWN.store(true, Ordering::SeqCst);
}

fn main() -> Result<(), GpsdError> {
    // Install signal handlers for graceful shutdown
    // This ensures LLVM coverage profraw files are flushed on SIGTERM/SIGINT
    install_signal_handlers();

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

    while !SHUTDOWN.load(Ordering::SeqCst) {
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
                handle_tpv(&session, &args.topic, &tpv);
                args.tracy.then(|| secondary_frame_mark!("tpv"));
            }
            ResponseData::Sky(sky) => handle_sky(sky),
            ResponseData::Pps(pps) => handle_pps(pps),
            ResponseData::Gst(gst) => {
                handle_gst(&session, &args.topic, &gst);
                args.tracy.then(|| secondary_frame_mark!("gst"));
            }
        }

        args.tracy.then(frame_mark);
    }

    info!("shutting down gracefully");
    Ok(())
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
fn handle_tpv(session: &Session, topic: &str, tpv: &gpsd_proto::Tpv) {
    debug!("{:?}", tpv);

    let stamp = match timestamp() {
        Ok(t) => t,
        Err(TimestampError::Overflow) => {
            warn!("Timestamp overflow: system clock exceeds i32 range (Y2038), saturating");
            edgefirst_schemas::builtin_interfaces::Time {
                sec: i32::MAX,
                nanosec: 999_999_999,
            }
        }
        Err(e) => {
            warn!("Failed to get timestamp: {}", e);
            return;
        }
    };

    let msg = create_navsat_fix_from_tpv(tpv, stamp);
    let msg = ZBytes::from(serialize(&msg).unwrap());
    let enc = Encoding::APPLICATION_CDR.with_schema(NavSatFix::SCHEMA_NAME);

    if let Err(e) = session.put(topic, msg).encoding(enc).wait() {
        warn!("Failed to publish TPV message: {}", e);
    }
}

#[instrument(skip_all)]
fn handle_gst(session: &Session, topic: &str, gst: &gpsd_proto::Gst) {
    debug!("{:?}", gst);

    let stamp = match timestamp() {
        Ok(t) => t,
        Err(TimestampError::Overflow) => {
            warn!("Timestamp overflow: system clock exceeds i32 range (Y2038), saturating");
            edgefirst_schemas::builtin_interfaces::Time {
                sec: i32::MAX,
                nanosec: 999_999_999,
            }
        }
        Err(e) => {
            warn!("Failed to get timestamp: {}", e);
            return;
        }
    };

    let msg = create_navsat_fix_from_gst(gst, stamp);
    let msg = ZBytes::from(serialize(&msg).unwrap());
    let enc = Encoding::APPLICATION_CDR.with_schema(NavSatFix::SCHEMA_NAME);

    if let Err(e) = session.put(topic, msg).encoding(enc).wait() {
        warn!("Failed to publish GST message: {}", e);
    }
}
