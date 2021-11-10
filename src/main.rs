#[macro_use]
extern crate log;

use std::env;
use std::io::Write;

use env_logger::Env;
use structopt::StructOpt;

mod api;
mod cli;
mod entities;
mod errors;
mod fingerprint;
mod path;
mod query;
mod storage;
mod tree;

use crate::cli::TmsuOptions;

fn main() {
    let opt = TmsuOptions::from_args();

    initialize_logging(opt.global_opts.verbose);

    // Parse CLI args and dispatch to the right subcommand
    let result = cli::run(opt);

    // If there is an error, print it and exit with a non-zero error code
    cli::print_error(result);
}

fn initialize_logging(force_verbose: bool) {
    // If --verbose is passed, set the logger to trace with a minimalistic formatter.
    // If the RUST_LOG environment variable is defined, respect it and use the default formatter.
    // Otherwise fallback onto "warn" level and a minimalistic formatter, to allow outputting
    // warnings on the console.

    let formatter = |buf: &mut env_logger::fmt::Formatter, record: &log::Record| {
        writeln!(buf, "tmsu: {}", record.args())
    };

    if force_verbose {
        let mut builder = env_logger::Builder::new();
        builder.filter_level(log::LevelFilter::Trace);
        builder.format(formatter);
        builder.init();
    } else if env::var("RUST_LOG").is_err() {
        let default_level = if cfg!(debug_assertions) {
            "tmsu=debug"
        } else {
            "tmsu=warn"
        };
        env_logger::from_env(Env::default().default_filter_or(default_level))
            .format(formatter)
            .init();
    } else {
        env_logger::init();
    }
}
