//! Top-level application flows: run, probe, restore, and validation.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use anyhow::{Context, Result, bail};
use log::{info, warn};

use crate::cli::RunArgs;
use crate::config::{Config, DeviceConfig, resolve_default_path};
use crate::device::Device;
use crate::device::g102;
use crate::device::registry;
use crate::device::sysfs;
use crate::effect::NAMES;
use crate::engine::Engine;
use crate::stage::Stage;
use crate::sync::Session;

pub fn run(config_path: Option<&Path>, args: RunArgs, stop: Arc<AtomicBool>) -> Result<()> {
    let mut config = load_config(config_path)?;
    apply_run_overrides(&mut config, &args)?;
    let engine = Engine::new(&config.effect)?;
    let selected = select_devices(&config, args.devices.as_deref())?;
    let stage = Stage::new(&config.stage);
    stage.validate_for(
        selected
            .iter()
            .map(|device| (device.id.as_str(), device.kind.as_str())),
    )?;
    let devices = open_devices(&selected, args.dry_run, args.no_restore, config.general.fps)?;

    info!(
        "effect={} color={} speed={} brightness={} fps={}",
        engine.name(),
        config.effect.color,
        config.effect.speed,
        config.effect.brightness,
        config.general.fps
    );
    for device in &selected {
        info!(
            "device {}: type={} required={} max_fps={}",
            device.id,
            device.kind,
            device.required,
            device
                .max_fps
                .unwrap_or_else(|| registry::recommended_fps(&device.kind))
        );
    }

    let session = Session::start(devices, &stage, Arc::clone(&stop), args.no_restore)?;
    let drive_result = crate::drive::drive(
        &session,
        &engine,
        config.general.fps,
        config.general.duration_secs,
        config.general.stats_interval_secs,
        config.general.on_error,
        &stop,
    );
    let shutdown_result = {
        let mut session = session;
        session.shutdown()
    };

    match (drive_result, shutdown_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(err), Ok(())) => Err(err),
        (Ok(()), Err(err)) => Err(err),
        (Err(err), Err(shutdown)) => {
            Err(err.context(format!("shutdown also failed: {shutdown:#}")))
        }
    }
}

pub fn list_devices() -> Result<()> {
    let devices = registry::discover();
    println!("supported devices:");
    for device in devices {
        let state = if device.present { "present" } else { "missing" };
        println!("  {:<9} {:<9} {}", device.kind, state, device.detail);
        if !device.present {
            println!("            model: {}", device.model);
            println!("            hint: {}", remediation(device.kind));
        }
    }
    Ok(())
}

pub fn list_effects() -> Result<()> {
    println!("effects:");
    for (name, description) in NAMES {
        println!("  {name:<11} {description}");
    }
    Ok(())
}

pub fn validate_config(config_path: Option<&Path>) -> Result<()> {
    let config = load_config(config_path)?;
    let path = config_path
        .map(Path::to_path_buf)
        .unwrap_or_else(resolve_default_path);
    println!("config: {}", path.display());
    println!(
        "effect: {} color={} speed={} brightness={} wave_cycles={}",
        config.effect.name,
        config.effect.color,
        config.effect.speed,
        config.effect.brightness,
        config.effect.wave_cycles
    );
    println!(
        "general: fps={} duration={:?} on_error={:?}",
        config.general.fps, config.general.duration_secs, config.general.on_error
    );
    println!("devices:");
    for device in &config.devices {
        println!(
            "  id={} type={} enabled={} required={} max_fps={:?} restore={:?}",
            device.id,
            device.kind,
            device.enabled,
            device.required,
            device.max_fps,
            device.restore.policy
        );
    }
    println!("stage loop:");
    if config.stage.devices.is_empty() {
        println!("  kind defaults: k617 0.00..0.45, g102 0.45..0.70, asus_tuf 0.70..0.85");
    } else {
        for stage in &config.stage.devices {
            println!("  {} {:.2}..{:.2}", stage.id, stage.start, stage.end);
        }
    }
    println!("config is valid");
    Ok(())
}

/// Apply the configured restore state without running an effect.
///
/// This is a recovery command. For the K617 it opens the device, captures the
/// current 136-byte config, and writes that same config back, which is enough
/// to leave a stale direct-per-key frame. The other two drivers apply their
/// configured default because their protocols have no color readback.
pub fn restore_defaults(config_path: Option<&Path>) -> Result<()> {
    let config = load_config(config_path)?;
    let selected = select_devices(&config, None)?;
    let mut devices = open_devices(&selected, false, false, config.general.fps)?;
    let mut errors = Vec::new();
    for (device, _) in &mut devices {
        match device.restore() {
            Ok(()) => info!("{}: restore applied", device.id()),
            Err(err) => {
                log::error!("{}: restore failed: {err:#}", device.id());
                errors.push(format!("{}: {err:#}", device.id()));
            }
        }
        device.close();
    }
    if errors.is_empty() {
        Ok(())
    } else {
        bail!("restore problems: {}", errors.join("; "))
    }
}

/// Read-only probe. Opens only the G102 HID++ channel; no lighting writes.
pub fn probe() -> Result<()> {
    list_devices()?;

    println!("\nG102 HID++ 2.0 feature table (read-only):");
    match sysfs::find_hidraw(g102::VENDOR_ID, g102::PRODUCT_ID, Some(1), Some(0x11)) {
        Ok(found) => match g102::probe_features(&found.path) {
            Ok(features) => {
                if features.is_empty() {
                    println!("  no feature IDs returned (device may need a moment after plug-in)");
                } else {
                    for (index, feature) in features {
                        println!("  index {index:#04x} -> feature {feature:#06x}");
                    }
                    println!(
                        "  note: the G203L lighting driver addresses feature index 0x0e directly"
                    );
                }
            }
            Err(err) => println!(
                "  G102 probe failed: {err:#}\n  hint: {}",
                remediation("g102")
            ),
        },
        Err(err) => println!("  G102 not available: {err:#}"),
    }

    if let Ok(base) = sysfs::find_asus_keyboard_led() {
        let brightness = std::fs::read_to_string(base.join("brightness")).unwrap_or_default();
        let max = std::fs::read_to_string(base.join("max_brightness")).unwrap_or_default();
        println!(
            "\nASUS keyboard LED: {} (brightness {}/{})",
            base.display(),
            brightness.trim(),
            max.trim()
        );
    }

    println!(
        "\nK617: the 136-byte config snapshot is read at run startup; \
         the per-key frame path is report 0x08"
    );
    Ok(())
}

pub(crate) fn load_config(path: Option<&Path>) -> Result<Config> {
    let (config, path) = Config::load(path)?;
    info!("loaded config {}", path.display());
    Ok(config)
}

fn apply_run_overrides(config: &mut Config, args: &RunArgs) -> Result<()> {
    if let Some(effect) = &args.effect {
        config.effect.name = effect.clone();
    }
    if let Some(color) = &args.color {
        config.effect.color = color
            .parse()
            .map_err(|err: String| anyhow::anyhow!("invalid --color: {err}"))?;
    }
    if let Some(speed) = args.speed {
        config.effect.speed = speed;
    }
    if let Some(brightness) = args.brightness {
        config.effect.brightness = brightness;
    }
    if let Some(fps) = args.fps {
        config.general.fps = fps;
    }
    if let Some(duration) = args.duration {
        config.general.duration_secs = Some(duration);
    }
    config.validate()
}

pub(crate) fn select_devices<'a>(
    config: &'a Config,
    filter: Option<&str>,
) -> Result<Vec<&'a DeviceConfig>> {
    match filter {
        Some(filter) => {
            let mut selected = Vec::new();
            for id in filter.split(',').map(str::trim).filter(|id| !id.is_empty()) {
                let device = config
                    .devices
                    .iter()
                    .find(|device| device.id == id)
                    .with_context(|| {
                        format!(
                            "unknown device id {id:?}; configured ids: {}",
                            config
                                .devices
                                .iter()
                                .map(|d| d.id.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    })?;
                if !device.enabled {
                    bail!("device {id:?} is disabled in the config");
                }
                if !selected
                    .iter()
                    .any(|existing: &&DeviceConfig| existing.id == id)
                {
                    selected.push(device);
                }
            }
            if selected.is_empty() {
                bail!("--devices did not name any device");
            }
            Ok(selected)
        }
        None => {
            let selected: Vec<_> = config.enabled_devices().collect();
            if selected.is_empty() {
                bail!("no enabled devices in the config");
            }
            Ok(selected)
        }
    }
}

pub(crate) fn open_devices(
    configs: &[&DeviceConfig],
    dry_run: bool,
    no_restore: bool,
    fps: u32,
) -> Result<Vec<(Box<dyn Device>, u32)>> {
    let mut opened: Vec<(Box<dyn Device>, u32)> = Vec::new();
    for config in configs {
        let mut device: Box<dyn Device> = if dry_run {
            Box::new(registry::mock_for(config)?)
        } else {
            registry::build(config)?
        };
        match device.open() {
            Ok(()) => {
                if !device.is_open() {
                    bail!(
                        "device {} ({}) reported a successful open but is_open() is false",
                        config.id,
                        config.kind
                    );
                }
                info!("opened {}: {}", device.id(), device.model());
                let max_fps = config
                    .max_fps
                    .unwrap_or_else(|| registry::recommended_fps(&config.kind).min(fps.max(1)));
                opened.push((device, max_fps));
            }
            Err(err) if !config.required => {
                warn!(
                    "skipping optional device {} ({}): {err:#}",
                    config.id, config.kind
                );
                device.close();
            }
            Err(err) => {
                for (mut opened_device, _) in opened.drain(..) {
                    if !no_restore {
                        let _ = opened_device.restore();
                    }
                    opened_device.close();
                }
                return Err(err).with_context(|| {
                    format!(
                        "opening device {} ({}); run `luma list-devices` for diagnostics",
                        config.id, config.kind
                    )
                });
            }
        }
    }
    if opened.is_empty() {
        bail!("no devices could be opened; check `luma list-devices`");
    }
    Ok(opened)
}

fn remediation(kind: &str) -> &'static str {
    match kind {
        "g102" | "k617" => {
            "install udev rules with `sudo scripts/install-permissions.sh`, then replug"
        }
        "asus_tuf" => {
            "kernel 6.2+ with asus_nb_wmi; install the udev rule and check `dmesg | grep -i asus`"
        }
        _ => "see README.md",
    }
}
