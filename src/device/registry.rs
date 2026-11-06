//! Driver registry and read-only discovery.
//!
//! Adding a device means adding a module, one match arm here, and a discovery
//! entry. No engine code changes.

use anyhow::{Context, Result, bail};

use crate::color::Rgb;
use crate::config::DeviceConfig;
use crate::device::asus_tuf::AsusTuf;
use crate::device::g102::G102;
use crate::device::k617::K617;
use crate::device::mock::MockDevice;
use crate::device::{Device, sysfs};

#[derive(Clone, Debug)]
pub struct Discovered {
    pub kind: &'static str,
    pub model: &'static str,
    pub present: bool,
    pub detail: String,
}

/// Build one driver from config. The caller opens it.
pub fn build(config: &DeviceConfig) -> Result<Box<dyn Device>> {
    let device: Box<dyn Device> = match config.kind.trim().to_ascii_lowercase().as_str() {
        "g102" => Box::new(G102::new(config).context("building G102 driver")?),
        "k617" => Box::new(K617::new(config).context("building K617 driver")?),
        "asus_tuf" | "asus-tuf" | "tuf" => {
            Box::new(AsusTuf::new(config).context("building ASUS TUF driver")?)
        }
        other => bail!(
            "unknown device type {other:?} for {}; supported: g102, k617, asus_tuf",
            config.id
        ),
    };
    Ok(device)
}

/// Default render rate for a driver. The ASUS EC is the slow path; 15 fps is
/// a clean divisor of the typical 30 fps engine clock and keeps ACPI traffic
/// sane without making a single-zone fade look stepped.
pub fn recommended_fps(kind: &str) -> u32 {
    match kind.trim().to_ascii_lowercase().as_str() {
        "asus_tuf" | "asus-tuf" | "tuf" => 15,
        _ => 30,
    }
}

/// Build an in-memory device with the same layout as the real driver.
pub fn mock_for(config: &DeviceConfig) -> Result<MockDevice> {
    let (layout, kind) = match config.kind.trim().to_ascii_lowercase().as_str() {
        "g102" => (G102::new(config)?.layout().to_vec(), "g102"),
        "k617" => (K617::new(config)?.layout().to_vec(), "k617"),
        "asus_tuf" | "asus-tuf" | "tuf" => (AsusTuf::new(config)?.layout().to_vec(), "asus_tuf"),
        other => bail!(
            "unknown device type {other:?} for {}; supported: g102, k617, asus_tuf",
            config.id
        ),
    };
    let restore_color = if crate::config::RestorePolicy::Off == config.restore.policy {
        Rgb::BLACK
    } else {
        config
            .restore
            .color
            .unwrap_or(Rgb::WHITE)
            .scale(config.restore.brightness.unwrap_or(0.3))
    };
    Ok(MockDevice::new(
        config.id.clone(),
        kind,
        layout,
        restore_color,
    ))
}

/// Enumerate the three supported devices without opening them.
pub fn discover() -> Vec<Discovered> {
    let mut result = Vec::new();

    result.push(discover_hidraw(
        "g102",
        "Logitech G102 LIGHTSYNC",
        0x046d,
        0xc092,
        1,
        0x11,
    ));
    result.push(discover_hidraw(
        "k617",
        "Redragon K617 Fizz",
        0x258a,
        0x0049,
        1,
        0x08,
    ));

    let asus = sysfs::find_asus_keyboard_led();
    result.push(match asus {
        Ok(path) => {
            let brightness = std::fs::read_to_string(path.join("max_brightness"))
                .ok()
                .map(|v| v.trim().to_string());
            Discovered {
                kind: "asus_tuf",
                model: "ASUS TUF A15 keyboard",
                present: true,
                detail: format!(
                    "{} (single zone{})",
                    path.display(),
                    brightness
                        .map(|v| format!(", max brightness {v}"))
                        .unwrap_or_default()
                ),
            }
        }
        Err(err) => Discovered {
            kind: "asus_tuf",
            model: "ASUS TUF A15 keyboard",
            present: false,
            detail: err.to_string(),
        },
    });

    result
}

fn discover_hidraw(
    kind: &'static str,
    model: &'static str,
    vendor: u16,
    product: u16,
    interface: u8,
    report_id: u8,
) -> Discovered {
    match sysfs::find_all_hidraw(vendor, product) {
        Ok(devices) => {
            let matches: Vec<_> = devices
                .into_iter()
                .filter(|dev| dev.interface == Some(interface))
                .filter(|dev| dev.has_report_id(report_id))
                .collect();
            match matches.len() {
                0 => Discovered {
                    kind,
                    model,
                    present: false,
                    detail: format!("no hidraw control interface for {vendor:04x}:{product:04x}"),
                },
                1 => {
                    let dev = &matches[0];
                    Discovered {
                        kind,
                        model,
                        present: true,
                        detail: format!(
                            "{} (interface {interface}, report {report_id:#04x})",
                            dev.path.display()
                        ),
                    }
                }
                _ => Discovered {
                    kind,
                    model,
                    present: true,
                    detail: format!(
                        "{} matching hidraw nodes: {}",
                        matches.len(),
                        matches
                            .iter()
                            .map(|d| d.path.display().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                },
            }
        }
        Err(err) => Discovered {
            kind,
            model,
            present: false,
            detail: err.to_string(),
        },
    }
}
