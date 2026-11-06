//! luma: one lighting clock for a Redragon K617, Logitech G102, and ASUS
//! TUF laptop keyboard.
//!
//! See README.md for the protocol evidence and docs/ARCHITECTURE.md for the
//! data flow. The code is organized so each device is one module and each
//! effect is one file.

mod app;
mod cli;
mod color;
mod config;
mod device;
mod drive;
mod effect;
mod engine;
mod geometry;
mod self_test;
mod stage;
mod sync;

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use anyhow::{Context, Result};
use clap::Parser;

use crate::cli::{Cli, Command};

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_logging(&cli.log);

    match execute(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn init_logging(level: &str) {
    let env = env_logger::Env::default().default_filter_or(level);
    env_logger::Builder::from_env(env)
        .format_timestamp_millis()
        .init();
}

fn execute(cli: Cli) -> Result<()> {
    let stop = Arc::new(AtomicBool::new(false));
    match cli.command {
        Command::Run(args) => {
            install_signal_handlers(&stop)?;
            app::run(cli.config.as_deref(), args, stop)
        }
        Command::SelfTest(args) => {
            install_signal_handlers(&stop)?;
            self_test::self_test(cli.config.as_deref(), args, stop)
        }
        Command::ListDevices => app::list_devices(),
        Command::Effects => app::list_effects(),
        Command::Probe => app::probe(),
        Command::ValidateConfig => app::validate_config(cli.config.as_deref()),
        Command::Restore => app::restore_defaults(cli.config.as_deref()),
    }
}

fn install_signal_handlers(stop: &Arc<AtomicBool>) -> Result<()> {
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signal_hook::flag::register(signal, Arc::clone(stop))
            .with_context(|| format!("cannot register handler for signal {signal}"))?;
    }
    Ok(())
}
