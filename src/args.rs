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

/// Names of this program's env-bound arguments whose value, as reported by
/// `var`, is present but empty and not listed in `keep`.
///
/// Pure: the environment is only read through `var`, so this can be unit
/// tested with a fake lookup and no process-wide mutation.
pub fn empty_env_vars<C: CommandFactory>(
    keep: &[&str],
    var: impl Fn(&str) -> Option<String>,
) -> Vec<String> {
    C::command()
        .get_arguments()
        .filter_map(|arg| arg.get_env().map(|e| e.to_string_lossy().into_owned()))
        .filter(|name| !keep.contains(&name.as_str()))
        .filter(|name| var(name).is_some_and(|v| v.is_empty()))
        .collect()
}

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
    for name in empty_env_vars::<C>(keep, |name| std::env::var(name).ok()) {
        std::env::remove_var(&name);
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
    use std::collections::HashMap;

    /// Env-bound arguments with a non-empty default where we have consciously
    /// decided that an empty value is NOT meaningful (so scrubbing to the
    /// default is correct).
    const SCRUB_REVIEWED: &[&str] = &["GPSD", "TOPIC", "RUST_LOG", "MODE"];

    fn parse_cli() -> Args {
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

    /// Fake environment lookup over a fixed table; never touches the process.
    fn lookup(env: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let table: HashMap<String, String> = env
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |name| table.get(name).cloned()
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
    fn empty_env_vars_lists_only_empty_bound_vars() {
        let env = [
            ("TRACY", ""),           // empty bool: listed
            ("RUST_LOG", ""),        // empty level: listed
            ("CONNECT", ""),         // empty vec: listed
            ("GPSD", "10.0.0.1:99"), // non-empty: not listed
            ("MODE", "peer"),        // non-empty: not listed
                                     // TOPIC, LISTEN, NO_MULTICAST_SCOUTING unset: not listed
        ];
        let mut got = empty_env_vars::<Args>(KEEP, lookup(&env));
        got.sort();
        assert_eq!(got, ["CONNECT", "RUST_LOG", "TRACY"]);
    }

    #[test]
    fn empty_env_vars_ignores_unset_and_nonempty() {
        let env = [("GPSD", "127.0.0.1:2947"), ("TOPIC", "gps")];
        assert!(empty_env_vars::<Args>(KEEP, lookup(&env)).is_empty());
        assert!(empty_env_vars::<Args>(KEEP, lookup(&[])).is_empty());
    }

    #[test]
    fn empty_env_vars_honours_keep() {
        let env = [("TRACY", ""), ("TOPIC", "")];
        // Same input, TOPIC excluded once it is kept.
        let mut got = empty_env_vars::<Args>(&[], lookup(&env));
        got.sort();
        assert_eq!(got, ["TOPIC", "TRACY"]);
        assert_eq!(empty_env_vars::<Args>(&["TOPIC"], lookup(&env)), ["TRACY"]);
        assert!(empty_env_vars::<Args>(&["TOPIC", "TRACY"], lookup(&env)).is_empty());
    }

    #[test]
    fn empty_env_vars_never_lists_unbound_vars() {
        // Present and empty, but not bound to any argument: left alone.
        let env = [("PATH", ""), ("HOME", ""), ("NOT_A_NAVSAT_VAR", "")];
        assert!(empty_env_vars::<Args>(KEEP, lookup(&env)).is_empty());
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
