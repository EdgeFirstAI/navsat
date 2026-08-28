// Copyright 2025 Au-Zone Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use serde_json::json;
use tracing::level_filters::LevelFilter;
use zenoh::config::{Config, WhatAmI};

/// Command-line arguments for EdgeFirst NavSat Node.
///
/// This structure defines all configuration options for the navsat node,
/// including GPSD connection, Zenoh configuration, logging, and debugging
/// options. Arguments can be specified via command line or environment
/// variables.
///
/// # Example
///
/// ```bash
/// # Via command line
/// edgefirst-navsat --gpsd 127.0.0.1:2947 --topic gps
///
/// # Via environment variables
/// export GPSD="127.0.0.1:2947"
/// export TOPIC="gps"
/// edgefirst-navsat
/// ```
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// GPSD daemon endpoint to connect to (host:port)
    #[arg(long, env = "GPSD", default_value = "127.0.0.1:2947")]
    pub gpsd: String,

    /// Zenoh key expression for NavSatFix messages.
    /// The session namespace prefixes this with `{hostname}/` on the wire.
    #[arg(long, env = "TOPIC", default_value = "gps")]
    pub topic: String,

    /// Application log level
    #[arg(long, env = "RUST_LOG", default_value = "info")]
    pub rust_log: LevelFilter,

    /// Enable Tracy profiler broadcast
    #[arg(long, env = "TRACY")]
    pub tracy: bool,

    /// Zenoh participant mode (peer, client, or router)
    #[arg(long, env = "MODE", default_value = "peer")]
    mode: WhatAmI,

    /// Zenoh endpoints to connect to (can specify multiple)
    #[arg(long, env = "CONNECT")]
    connect: Vec<String>,

    /// Zenoh endpoints to listen on (can specify multiple)
    #[arg(long, env = "LISTEN")]
    listen: Vec<String>,

    /// Disable Zenoh multicast peer discovery
    #[arg(long, env = "NO_MULTICAST_SCOUTING")]
    no_multicast_scouting: bool,
}

/// System hostname used as the Zenoh session namespace.
///
/// Empty or `/`-containing hostnames would create unintended sub-keys, so we
/// fall back to `"localhost"` and warn. Two devices both falling back would
/// silently share a namespace; that is a deployment defect.
fn zenoh_namespace() -> String {
    let raw = gethostname::gethostname().to_string_lossy().into_owned();
    if raw.is_empty() || raw.contains('/') {
        tracing::warn!(
            hostname = %raw,
            "system hostname is empty or contains '/' — falling back to \"localhost\""
        );
        "localhost".into()
    } else {
        raw
    }
}

impl From<Args> for Config {
    fn from(args: Args) -> Self {
        let mut config = Config::default();

        // Session namespace = hostname: application keys are bare (`gps`)
        // and the wire form is `{hostname}/gps`.
        config
            .insert_json5("namespace", &json!(zenoh_namespace()).to_string())
            .unwrap();

        config
            .insert_json5("mode", &json!(args.mode).to_string())
            .unwrap();

        let connect: Vec<_> = args.connect.into_iter().filter(|s| !s.is_empty()).collect();
        if !connect.is_empty() {
            config
                .insert_json5("connect/endpoints", &json!(connect).to_string())
                .unwrap();
        }

        let listen: Vec<_> = args.listen.into_iter().filter(|s| !s.is_empty()).collect();
        if !listen.is_empty() {
            config
                .insert_json5("listen/endpoints", &json!(listen).to_string())
                .unwrap();
        }

        if args.no_multicast_scouting {
            config
                .insert_json5("scouting/multicast/enabled", &json!(false).to_string())
                .unwrap();
        }

        config
            .insert_json5("scouting/multicast/interface", &json!("lo").to_string())
            .unwrap();

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn zenoh_config_sets_namespace() {
        let args = Args::parse_from(["edgefirst-navsat"]);
        let cfg = Config::from(args);
        let ns: String = serde_json::from_str(&cfg.to_string())
            .ok()
            .and_then(|v: serde_json::Value| {
                v.pointer("/namespace")
                    .and_then(|n| n.as_str().map(String::from))
            })
            .expect("namespace should be set in config");
        assert!(!ns.is_empty(), "namespace should be non-empty");
        assert!(!ns.contains('/'), "namespace must not contain '/'");
    }

    #[test]
    fn default_topic_has_no_rt_prefix() {
        let args = Args::parse_from(["edgefirst-navsat"]);
        assert_eq!(args.topic, "gps");
    }
}
