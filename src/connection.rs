use zenoh::{config::Config, prelude::r#async::*};
use std::{str::FromStr, future::Ready};

pub fn start_session(mode: &str, endpoint: &Vec<std::string::String>) -> Ready<Result<Session, Box<(dyn std::error::Error + std::marker::Send + Sync + 'static)>>>{
    let mut config = Config::default();
    let mode = WhatAmI::from_str(mode).unwrap();
    config.set_mode(Some(mode)).unwrap();
    config.connect.endpoints = endpoint.iter().map(|v| v.parse().unwrap()).collect();
    return zenoh::open(config).res();
}