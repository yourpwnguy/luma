//! ASUS TUF A15 keyboard backlight (`asus-wmi` ABI).
//!
//! The EC interface is a set of sysfs LED attributes, not USB HID:
//!
//! ```text
//! /sys/class/leds/asus::kbd_backlight/kbd_rgb_mode
//! /sys/class/leds/asus::kbd_backlight/brightness
//! ```
//!
//! `kbd_rgb_mode` takes six decimal fields: `save mode r g b speed`.
//! The kernel maps `save=0` to command `0xb3` (temporary) and `save=1` to
//! `0xb4` (save to BIOS). We always use `save=0`. We also never touch
//! `kbd_rgb_state`, which controls boot/sleep/shutdown lighting and is
//! persistent. The FA506 A15 exposes one logical zone, so effects are
//! rendered centrally and written as a static color at the frame rate.
//!
//! Source of truth for the ABI: Linux `drivers/platform/x86/asus-wmi.c`,
//! `kbd_rgb_mode_store()`; the sysfs `_index` file on this machine confirms
//! the same field order.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use log::{debug, info, warn};

use crate::color::Rgb;
use crate::config::{DeviceConfig, RestorePolicy};
use crate::device::sysfs::{find_asus_keyboard_led, permission_hint};
use crate::device::{Device, ensure_frame_len};
use crate::geometry::LedSpec;

pub const MODEL: &str = "ASUS TUF Gaming A15 keyboard";
const DEFAULT_MAX_BRIGHTNESS: u8 = 3;

pub struct AsusTuf {
    id: String,
    base: Option<PathBuf>,
    layout: Vec<LedSpec>,
    opened: bool,
    max_brightness: u8,
    snapshot_brightness: Option<u8>,
    run_brightness: Option<u8>,
    restore_policy: RestorePolicy,
    restore_color: Rgb,
    restore_hardware_brightness: Option<u8>,
    last_color: Option<Rgb>,
}

impl AsusTuf {
    pub fn new(config: &DeviceConfig) -> Result<Self> {
        Self::with_base(config, None)
    }

    fn with_base(config: &DeviceConfig, base: Option<PathBuf>) -> Result<Self> {
        let restore_color = if config.restore.policy == RestorePolicy::Off {
            Rgb::BLACK
        } else {
            config
                .restore
                .color
                .unwrap_or(Rgb::WHITE)
                .scale(config.restore.brightness.unwrap_or(1.0))
        };
        Ok(Self {
            id: config.id.clone(),
            base,
            layout: vec![LedSpec::new("keyboard", 0.5, 0.5)],
            opened: false,
            max_brightness: DEFAULT_MAX_BRIGHTNESS,
            snapshot_brightness: None,
            run_brightness: config.option_u8("hardware_brightness")?,
            restore_policy: config.restore.policy,
            restore_color,
            restore_hardware_brightness: config.restore.hardware_brightness,
            last_color: None,
        })
    }

    fn base(&self) -> Result<&Path> {
        self.base
            .as_deref()
            .context("ASUS LED base path has not been resolved; call open() first")
    }

    fn mode_path(&self) -> Result<PathBuf> {
        Ok(self.base()?.join("kbd_rgb_mode"))
    }

    fn brightness_path(&self) -> Result<PathBuf> {
        Ok(self.base()?.join("brightness"))
    }

    fn write_color(&self, color: Rgb) -> Result<()> {
        let path = self.mode_path()?;
        let command = mode_command(color);
        fs::write(&path, command.as_bytes()).map_err(|err| {
            if err.kind() == std::io::ErrorKind::PermissionDenied {
                anyhow::anyhow!(permission_hint(&path))
            } else {
                anyhow::anyhow!("cannot write {}: {err}", path.display())
            }
        })?;
        Ok(())
    }

    fn write_brightness(&self, level: u8) -> Result<()> {
        let path = self.brightness_path()?;
        let level = level.min(self.max_brightness);
        fs::write(&path, level.to_string().as_bytes()).map_err(|err| {
            if err.kind() == std::io::ErrorKind::PermissionDenied {
                anyhow::anyhow!(permission_hint(&path))
            } else {
                anyhow::anyhow!("cannot write {}: {err}", path.display())
            }
        })?;
        Ok(())
    }
}

impl Device for AsusTuf {
    fn id(&self) -> &str {
        &self.id
    }

    fn kind(&self) -> &'static str {
        "asus_tuf"
    }

    fn model(&self) -> &'static str {
        MODEL
    }

    fn layout(&self) -> &[LedSpec] {
        &self.layout
    }

    fn open(&mut self) -> Result<()> {
        if self.base.is_none() {
            self.base = Some(find_asus_keyboard_led()?);
        }
        let base = self.base()?.to_path_buf();
        let mode_path = base.join("kbd_rgb_mode");
        if !mode_path.is_file() {
            bail!(
                "{} does not exist; this kernel does not expose the TUF RGB ABI",
                mode_path.display()
            );
        }

        if let Ok(text) = fs::read_to_string(base.join("max_brightness")) {
            self.max_brightness = text.trim().parse().unwrap_or(DEFAULT_MAX_BRIGHTNESS);
        }
        // Only capture the original brightness on the first open. A reconnect
        // must not replace the snapshot with the brightness we set ourselves.
        if self.snapshot_brightness.is_none() {
            if let Ok(text) = fs::read_to_string(base.join("brightness")) {
                self.snapshot_brightness = text.trim().parse::<u8>().ok();
            }
        }
        let level = self.run_brightness.unwrap_or(self.max_brightness);
        if level > self.max_brightness {
            bail!(
                "{}: hardware_brightness {level} exceeds max_brightness {}",
                self.id,
                self.max_brightness
            );
        }
        self.write_brightness(level)?;
        if level == 0 {
            warn!(
                "{}: hardware brightness is 0; the effect will not be visible",
                self.id
            );
        }
        self.opened = true;
        self.last_color = None;
        info!(
            "{}: opened {} (max brightness {})",
            self.id,
            base.display(),
            self.max_brightness
        );
        Ok(())
    }

    fn send(&mut self, colors: &[Rgb]) -> Result<()> {
        ensure_frame_len(colors, 1)?;
        let color = colors[0];
        if self.last_color == Some(color) {
            return Ok(());
        }
        if !self.opened {
            bail!("{}: ASUS device is not open", self.id);
        }
        self.write_color(color)?;
        self.last_color = Some(color);
        Ok(())
    }

    fn restore(&mut self) -> Result<()> {
        if self.restore_policy == RestorePolicy::Leave {
            debug!("{}: restore policy is leave", self.id);
            return Ok(());
        }
        if self.base.is_none() {
            return Ok(());
        }
        let color = match self.restore_policy {
            RestorePolicy::Off => Rgb::BLACK,
            RestorePolicy::DefaultColor | RestorePolicy::Previous => self.restore_color,
            RestorePolicy::Leave => return Ok(()),
        };
        debug!("{}: restoring color {}", self.id, color.to_hex());
        self.write_color(color)?;
        let brightness = self
            .restore_hardware_brightness
            .or(self.snapshot_brightness)
            .unwrap_or(self.max_brightness);
        if let Err(err) = self.write_brightness(brightness) {
            warn!("{}: could not restore brightness: {err:#}", self.id);
        }
        Ok(())
    }

    fn close(&mut self) {
        self.opened = false;
    }

    fn is_open(&self) -> bool {
        self.opened
    }
}

/// Format the six-field sysfs command. `save=0` means temporary; mode 0 is
/// static; speed 1 is "normal" but is ignored by the static mode.
fn mode_command(color: Rgb) -> String {
    format!("0 0 {} {} {} 1", color.r, color.g, color.b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_base() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("luma-asus-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("kbd_rgb_mode"), b"").unwrap();
        fs::write(path.join("brightness"), b"2").unwrap();
        fs::write(path.join("max_brightness"), b"3").unwrap();
        path
    }

    #[test]
    fn command_uses_temporary_static_mode() {
        assert_eq!(mode_command(Rgb::new(1, 2, 3)), "0 0 1 2 3 1");
    }

    #[test]
    fn writes_color_and_brightness_then_restores() {
        let base = temp_base();
        let config: DeviceConfig = toml::from_str(
            r##"
            id = "laptop"
            type = "asus_tuf"
            [restore]
            policy = "default"
            color = "#ffffff"
            hardware_brightness = 1
            "##,
        )
        .unwrap();
        let mut device = AsusTuf::with_base(&config, Some(base.clone())).unwrap();
        device.open().unwrap();
        assert_eq!(fs::read_to_string(base.join("brightness")).unwrap(), "3");
        device.send(&[Rgb::new(10, 20, 30)]).unwrap();
        assert_eq!(
            fs::read_to_string(base.join("kbd_rgb_mode")).unwrap(),
            "0 0 10 20 30 1"
        );
        device.restore().unwrap();
        assert_eq!(
            fs::read_to_string(base.join("kbd_rgb_mode")).unwrap(),
            "0 0 255 255 255 1"
        );
        assert_eq!(fs::read_to_string(base.join("brightness")).unwrap(), "1");
        fs::remove_dir_all(base).unwrap();
    }
}
