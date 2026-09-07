// Copyright 2025 Au-Zone Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

//! End-to-end check that `KEY=""` in the environment behaves as unset.
//!
//! Runs with `harness = false` so this `main` is the only thread in the
//! process when the environment is mutated, which `scrub_empty_env` requires.
//!
//! Because there is no libtest harness, this binary implements the minimal
//! protocol cargo-nextest (used by CI and `make test`) expects from a custom
//! harness: `--list --format terse` prints one `<name>: test` line per test,
//! `--list --ignored` prints nothing, and any other invocation runs the test.

use clap::Parser;
use edgefirst_navsat::args::{scrub_empty_env, Args, KEEP};
use tracing::level_filters::LevelFilter;

/// Name reported to nextest / libtest-style listers.
const TEST_NAME: &str = "empty_env_is_treated_as_unset";

/// Env-bound arguments exercised here: a boolean, a typed level with a
/// non-empty default, an enum with a non-empty default, and a string with a
/// non-empty default. `KEEP` is empty for navsat, so all four are scrubbed.
const VARS: [&str; 4] = ["TRACY", "RUST_LOG", "MODE", "GPSD"];
const ARGV: [&str; 1] = ["edgefirst-navsat"];

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.iter().any(|a| a == "--list") {
        // Ignored-test listing: this harness has none.
        if !argv.iter().any(|a| a == "--ignored") {
            println!("{TEST_NAME}: test");
        }
        return;
    }

    for name in VARS {
        // Single-threaded: this is `main` before any thread is spawned.
        std::env::set_var(name, "");
    }
    let before = Args::try_parse_from(ARGV);
    assert!(
        before.is_err(),
        "empty vars must fail to parse before scrubbing: {before:?}"
    );

    // SAFETY: still single-threaded; no thread has been spawned in this
    // process, so nothing else can observe the environment mutation.
    unsafe { scrub_empty_env::<Args>(KEEP) };

    for name in VARS {
        assert!(
            std::env::var_os(name).is_none(),
            "{name} should have been removed"
        );
    }
    let args = Args::try_parse_from(ARGV).expect("defaults must apply after scrubbing");
    assert!(!args.tracy);
    assert_eq!(args.rust_log, LevelFilter::INFO);
    assert_eq!(args.gpsd, "127.0.0.1:2947");
    println!("env_scrub: ok");
}
