// Copyright 2025 Au-Zone Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

//! Maivin NavSat Library
//!
//! This library provides GPS/GNSS positioning functionality for the EdgeFirst Maivin platform.
//! It integrates with GPSD for GPS data and publishes NavSatFix messages via Zenoh.

pub mod args;
pub mod navsat;

pub use args::Args;
pub use navsat::{create_navsat_fix_from_gst, create_navsat_fix_from_tpv, timestamp};
