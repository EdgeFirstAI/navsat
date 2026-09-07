// Copyright 2025 Au-Zone Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

use clap::{CommandFactory, Parser};
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

/// Environment variables where an empty value is meaningful and must be
/// preserved (i.e. the argument has a non-empty default but "" is a
/// documented "disable" sentinel). NavSat has none: every empty value in
/// `navsat.default` means "use the default".
pub const KEEP: &[&str] = &[];

/// Treat an empty environment variable as unset, so clap's declared
/// `default_value` applies instead of failing to parse.
///
/// Only variables bound to this program's own arguments are considered;
/// unrelated process environment is left alone. `keep` names variables
/// where an empty value is meaningful and must be preserved.
///
/// # Safety
/// Must be called before any thread is spawned — that is, before the Zenoh
/// session is opened. Mutating the process environment is not thread-safe.
pub unsafe fn scrub_empty_env<C: CommandFactory>(keep: &[&str]) {
    for arg in C::command().get_arguments() {
        let Some(env) = arg.get_env() else { continue };
        let name = env.to_string_lossy().into_owned();
        if keep.contains(&name.as_str()) {
            continue;
        }
        if matches!(std::env::var(&name), Ok(v) if v.is_empty()) {
            std::env::remove_var(&name);
        }
    }
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
    use std::sync::{Mutex, MutexGuard};

    /// Serialises tests that read or mutate the process environment.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn env_lock() -> MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Env-bound arguments with a non-empty default where we have consciously
    /// decided that an empty value is NOT meaningful (so scrubbing to the
    /// default is correct).
    const SCRUB_REVIEWED: &[&str] = &["GPSD", "TOPIC", "RUST_LOG", "MODE"];

    fn parse_cli() -> Args {
        let _guard = env_lock();
        Args::parse_from([
            "edgefirst-navsat",
            "--topic",
            "gps",
            "--rust-log",
            "info",
            "--mode",
            "peer",
        ])
    }

    #[test]
    fn every_env_arg_is_either_scrubbable_or_explicitly_kept() {
        use clap::CommandFactory;
        for arg in Args::command().get_arguments() {
            let Some(env) = arg.get_env() else { continue };
            let name = env.to_string_lossy().into_owned();
            let has_nonempty_default = arg
                .get_default_values()
                .first()
                .is_some_and(|d| !d.is_empty());
            if has_nonempty_default && !KEEP.contains(&name.as_str()) {
                assert!(
                    SCRUB_REVIEWED.contains(&name.as_str()),
                    "{name} has a non-empty default; decide whether empty is meaningful \
                     and add it to KEEP or SCRUB_REVIEWED"
                );
            }
        }
    }

    #[test]
    fn empty_env_is_treated_as_unset() {
        let _guard = env_lock();
        let saved: Vec<_> = ["TRACY", "RUST_LOG", "CONNECT"]
            .into_iter()
            .map(|k| (k, std::env::var_os(k)))
            .collect();

        std::env::set_var("TRACY", "");
        std::env::set_var("RUST_LOG", "");
        std::env::set_var("CONNECT", "");

        // Without scrubbing, clap sees "" and fails to parse the bool / level.
        assert!(Args::try_parse_from(["edgefirst-navsat"]).is_err());

        // SAFETY: the environment lock is held and no other test thread
        // mutates the environment.
        unsafe { scrub_empty_env::<Args>(KEEP) };

        let result = Args::try_parse_from(["edgefirst-navsat"]);
        for (k, v) in saved {
            match v {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }

        let args = result.expect("empty env vars must fall back to defaults");
        assert!(!args.tracy);
        assert_eq!(args.rust_log, LevelFilter::INFO);
        assert!(args.connect.is_empty());
    }

    #[test]
    fn zenoh_config_sets_namespace() {
        let ns = zenoh_namespace();
        assert!(!ns.is_empty(), "namespace should be non-empty");
        assert!(!ns.contains('/'), "namespace must not contain '/'");
        let rendered = Config::from(parse_cli()).to_string();
        assert!(
            rendered.contains(&ns),
            "config should include namespace {ns}: {rendered}"
        );
    }

    #[test]
    fn cli_topic_is_gps() {
        assert_eq!(parse_cli().topic, "gps");
    }
}
