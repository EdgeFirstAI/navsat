use std::{str::FromStr, string::String};
use zenoh::{config::Config, prelude::sync::*};

pub fn start_session(
    mode: &str,
    endpoint: &[String],
) -> Result<Session, Box<(dyn std::error::Error + std::marker::Send + Sync + 'static)>> {
    let mut config = Config::default();
    let mode = WhatAmI::from_str(mode).unwrap();
    config.set_mode(Some(mode)).unwrap();
    config.connect.endpoints = endpoint.iter().map(|v| v.parse().unwrap()).collect();
    zenoh::open(config).res()
}
