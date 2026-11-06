//! Command-line interface.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "luma",
    version,
    about = "Synchronize RGB lighting across a Redragon K617, Logitech G102, and ASUS TUF laptop",
    long_about = None
)]
pub struct Cli {
    /// Log level: error, warn, info, debug, or trace.
    #[arg(long, global = true, default_value = "info", value_name = "LEVEL")]
    pub log: String,

    /// Config file. Defaults to ./config/luma.toml, then XDG config.
    #[arg(short, long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run an effect until Ctrl-C or --duration expires.
    Run(RunArgs),
    /// List supported devices and the nodes they resolve to.
    ListDevices,
    /// Print the available effects.
    Effects,
    /// Read-only probe of the connected devices.
    Probe,
    /// Show visible test patterns. Use --keymap to verify K617 LED slots.
    SelfTest(SelfTestArgs),
    /// Parse and validate the config, then exit.
    ValidateConfig,
    /// Open the devices and apply their configured restore state now.
    Restore,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    /// Effect name: static, off, breathing, rainbow, wave, cycle.
    #[arg(long, value_name = "NAME")]
    pub effect: Option<String>,

    /// Base color as #RRGGBB.
    #[arg(long, value_name = "HEX")]
    pub color: Option<String>,

    /// Speed multiplier. 1.0 is the effect's natural speed.
    #[arg(long, value_name = "FACTOR")]
    pub speed: Option<f32>,

    /// Overall brightness, 0.0..=1.0.
    #[arg(long, value_name = "LEVEL")]
    pub brightness: Option<f32>,

    /// Render rate in frames per second.
    #[arg(long, value_name = "FPS")]
    pub fps: Option<u32>,

    /// Stop after this many seconds instead of running until Ctrl-C.
    #[arg(long, value_name = "SECONDS")]
    pub duration: Option<f64>,

    /// Only use these config device ids, comma separated.
    #[arg(long, value_name = "ID,ID")]
    pub devices: Option<String>,

    /// Run the full engine with in-memory devices; no hardware access.
    #[arg(long)]
    pub dry_run: bool,

    /// Do not restore lighting on exit (the last frame stays visible).
    #[arg(long)]
    pub no_restore: bool,
}

#[derive(Debug, Args)]
pub struct SelfTestArgs {
    /// Only test this config device id.
    #[arg(long, value_name = "ID")]
    pub device: Option<String>,

    /// Sweep every K617 key one at a time so you can verify the slot map.
    #[arg(long)]
    pub keymap: bool,

    /// Light only these K617 keys, comma separated (for example
    /// Escape,Menu,RCtrl). Useful for checking the two unverified slots.
    #[arg(long, value_name = "NAME,NAME")]
    pub keys: Option<String>,

    /// Use in-memory devices; no hardware access.
    #[arg(long)]
    pub dry_run: bool,

    /// Do not restore lighting on exit.
    #[arg(long)]
    pub no_restore: bool,
}
