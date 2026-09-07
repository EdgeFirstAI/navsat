// Copyright 2025 Au-Zone Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

//! End-to-end check that `KEY=""` in the environment behaves as unset.
//!
//! Runs with `harness = false` so this `main` is the only thread in the
//! process when the environment is mutated, which `scrub_empty_env` requires.
//!
//! `main` speaks the small subset of the libtest CLI that `cargo test` and
//! `cargo nextest` use to enumerate (`--list --format terse`) and select
//! (`--exact <name>`, `--ignored`, `--skip <pattern>`, positional filters)
//! tests, so the target is discovered and reported like any other test.

use clap::Parser;
use edgefirst_navsat::args::{scrub_empty_env, Args, KEEP};
use tracing::level_filters::LevelFilter;

/// The single test this binary provides, as reported to the harness.
const TEST_NAME: &str = "empty_env_is_treated_as_unset";

/// Env-bound arguments exercised here: a boolean, a typed level with a
/// non-empty default, an enum with a non-empty default, and a string with a
/// non-empty default. `KEEP` is empty for navsat, so all four are scrubbed.
const VARS: [&str; 4] = ["TRACY", "RUST_LOG", "MODE", "GPSD"];
const ARGV: [&str; 1] = ["edgefirst-navsat"];

/// libtest flags that consume the following argument, so it is not a filter.
const VALUE_FLAGS: [&str; 6] = [
    "--test-threads",
    "--format",
    "--skip",
    "--logfile",
    "--color",
    "--shuffle-seed",
];

/// What the harness asked this binary to do.
struct Request {
    list: bool,
    ignored: bool,
    exact: bool,
    filters: Vec<String>,
    skips: Vec<String>,
}

fn parse_request(argv: impl IntoIterator<Item = String>) -> Request {
    let mut req = Request {
        list: false,
        ignored: false,
        exact: false,
        filters: Vec::new(),
        skips: Vec::new(),
    };
    let mut argv = argv.into_iter();
    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--list" => req.list = true,
            "--ignored" => req.ignored = true,
            "--exact" => req.exact = true,
            "--skip" => req.skips.extend(argv.next()),
            flag if VALUE_FLAGS.contains(&flag) => {
                argv.next();
            }
            flag if flag.starts_with('-') => {
                // `--flag=value` form; only `--skip=<pattern>` matters here.
                if let Some(pattern) = flag.strip_prefix("--skip=") {
                    req.skips.push(pattern.to_owned());
                }
            }
            filter => req.filters.push(filter.to_owned()),
        }
    }
    req
}

/// libtest matching: substring by default, equality under `--exact`.
fn matches(req: &Request, pattern: &str) -> bool {
    if req.exact {
        pattern == TEST_NAME
    } else {
        TEST_NAME.contains(pattern)
    }
}

/// Selected iff it passes the positional filter (any match, or no filters)
/// and no `--skip` pattern matches.
fn selected(req: &Request) -> bool {
    let filtered_in = req.filters.is_empty() || req.filters.iter().any(|f| matches(req, f));
    filtered_in && !req.skips.iter().any(|s| matches(req, s))
}

fn main() {
    let req = parse_request(std::env::args().skip(1));
    // This binary has no #[ignore]d tests, so `--ignored` selects nothing.
    if req.list {
        if !req.ignored && selected(&req) {
            println!("{TEST_NAME}: test");
        }
        return;
    }
    if req.ignored || !selected(&req) {
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
