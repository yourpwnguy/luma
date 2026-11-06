//! Logitech G102 LIGHTSYNC (USB `046d:c092`, also sold as G203 LIGHTSYNC).
//!
//! This is the vendor "G203L" lighting protocol, not the newer HID++ 2.0
//! COLOR_LED_EFFECTS feature. OpenRGB's G203L driver has used these exact
//! 20-byte report `0x11` commands on `c092` for years, and the packet layout
//! is also visible in the device's HID report descriptor. We only ever send
//! LED/direct-mode commands; there is no firmware or profile-flash path here.
//!
//! The mouse has three logical zones (left, center, right). The hardware LED
//! ids are 1, 2, 3 in that order. Direct mode is enabled by the magic
//! `0x0e/0x50` packet at open; each frame then writes all three colors and an
//! apply packet.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use hidapi::HidDevice;
use log::{debug, info};

use crate::color::Rgb;
use crate::config::{DeviceConfig, RestorePolicy};
use crate::device::sysfs::{find_hidraw, permission_hint};
use crate::device::{Device, drain_input, ensure_frame_len, open_hidraw};
use crate::geometry::LedSpec;

pub const VENDOR_ID: u16 = 0x046d;
pub const PRODUCT_ID: u16 = 0xc092;
pub const MODEL: &str = "Logitech G102 LIGHTSYNC";

const REPORT_ID: u8 = 0x11;
const PACKET_LEN: usize = 20;
const ZONES: usize = 3;

pub struct G102 {
    id: String,
    path: Option<PathBuf>,
    device: Option<HidDevice>,
    layout: Vec<LedSpec>,
    restore_policy: RestorePolicy,
    restore_color: Rgb,
    primed: bool,
}

impl G102 {
    pub fn new(config: &DeviceConfig) -> Result<Self> {
        let layout = vec![
            LedSpec::new("left", 0.0, 0.5),
            LedSpec::new("center", 0.5, 0.5),
            LedSpec::new("right", 1.0, 0.5),
        ];
        let restore_color = configured_restore_color(config, Rgb::WHITE, 0.30);
        Ok(Self {
            id: config.id.clone(),
            path: None,
            device: None,
            layout,
            restore_policy: config.restore.policy,
            restore_color,
            primed: false,
        })
    }

    fn write_packet(&self, packet: &[u8]) -> Result<()> {
        let device = self
            .device
            .as_ref()
            .context("G102 is not open; call open() first")?;
        let written = device.write(packet).with_context(|| {
            format!(
                "G102 write failed on {}",
                self.path
                    .as_ref()
                    .map_or_else(|| "<unknown>".to_string(), |p| p.display().to_string())
            )
        })?;
        if written != packet.len() {
            anyhow::bail!("G102 short write: {written} of {} bytes", packet.len());
        }
        drain_input(device);
        Ok(())
    }
}

impl Device for G102 {
    fn id(&self) -> &str {
        &self.id
    }

    fn kind(&self) -> &'static str {
        "g102"
    }

    fn model(&self) -> &'static str {
        MODEL
    }

    fn layout(&self) -> &[LedSpec] {
        &self.layout
    }

    fn open(&mut self) -> Result<()> {
        let found = find_hidraw(VENDOR_ID, PRODUCT_ID, Some(1), Some(REPORT_ID))
            .with_context(|| format!("device {}: {}", self.id, MODEL))?;
        let device = open_hidraw(&found.path).map_err(|err| {
            if err.to_string().contains("permission") {
                anyhow::anyhow!(permission_hint(&found.path))
            } else {
                err
            }
        })?;
        self.path = Some(found.path.clone());
        self.device = Some(device);
        self.primed = false;

        self.write_packet(&build_init_packet())?;
        info!("{}: opened {} in direct LED mode", self.id, found.summary());
        Ok(())
    }

    fn send(&mut self, colors: &[Rgb]) -> Result<()> {
        ensure_frame_len(colors, ZONES)?;
        let colors = [colors[0], colors[1], colors[2]];
        let color_packet = build_set_device_packet(&colors);
        let apply_packet = build_apply_packet();

        // OpenRGB sends the first frame twice because this firmware sometimes
        // ignores the first direct-mode update after the mode switch. Doing it
        // only on the first frame keeps the steady-state cost at two writes.
        if !self.primed {
            self.write_packet(&color_packet)?;
            self.write_packet(&apply_packet)?;
            self.primed = true;
        }
        self.write_packet(&color_packet)?;
        self.write_packet(&apply_packet)?;
        Ok(())
    }

    fn restore(&mut self) -> Result<()> {
        if self.restore_policy == RestorePolicy::Leave {
            debug!("{}: restore policy is leave", self.id);
            return Ok(());
        }
        if self.device.is_none() {
            self.open()?;
        }
        debug!("{}: restoring to {}", self.id, self.restore_color.to_hex());
        let colors = [self.restore_color; ZONES];
        self.send(&colors)
    }

    fn close(&mut self) {
        self.device = None;
        self.primed = false;
    }

    fn is_open(&self) -> bool {
        self.device.is_some()
    }
}

/// Read-only HID++ 2.0 feature table probe.
///
/// The G203L lighting commands use feature index `0x0e` directly, but this
/// enumeration lets us confirm what that index actually is on the attached
/// firmware and is useful when a future revision moves it. It only sends
/// `root.getFeature` requests; it never enables software control or writes
/// lighting state.
pub fn probe_features(path: &Path) -> Result<Vec<(u8, u16)>> {
    let device = open_hidraw(path)?;
    let mut features = Vec::new();
    for index in 0..=0x20u8 {
        let mut request = [0u8; PACKET_LEN];
        request[0] = REPORT_ID;
        request[1] = 0xff;
        request[2] = 0x00; // root feature index
        request[3] = 0x0a; // function 0, software id 0xA
        request[4] = index;
        let written = device
            .write(&request)
            .with_context(|| format!("HID++ getFeature request for index {index:#04x} failed"))?;
        if written != request.len() {
            anyhow::bail!("HID++ short write for feature index {index:#04x}");
        }
        let mut response = [0u8; PACKET_LEN];
        let read = device
            .read_timeout(&mut response, 20)
            .with_context(|| format!("HID++ reply for feature index {index:#04x} failed"))?;
        if read < 6 || response[0] != REPORT_ID || response[1] != 0xff {
            continue;
        }
        if response[3] & 0x80 != 0 {
            // HID++ error reply: this feature index is not present.
            continue;
        }
        let feature_id = u16::from_be_bytes([response[4], response[5]]);
        if feature_id != 0 {
            features.push((index, feature_id));
        }
    }
    Ok(features)
}

fn configured_restore_color(config: &DeviceConfig, default: Rgb, default_brightness: f32) -> Rgb {
    if config.restore.policy == RestorePolicy::Off {
        return Rgb::BLACK;
    }
    let color = config.restore.color.unwrap_or(default);
    let brightness = config.restore.brightness.unwrap_or(default_brightness);
    color.scale(brightness)
}

fn build_init_packet() -> [u8; PACKET_LEN] {
    let mut packet = [0u8; PACKET_LEN];
    packet[0] = REPORT_ID;
    packet[1] = 0xff;
    packet[2] = 0x0e;
    packet[3] = 0x50;
    packet[4] = 0x01;
    packet[5] = 0x03;
    packet[6] = 0x07;
    packet
}

fn build_set_device_packet(colors: &[Rgb; ZONES]) -> [u8; PACKET_LEN] {
    let mut packet = [0u8; PACKET_LEN];
    packet[0] = REPORT_ID;
    packet[1] = 0xff;
    packet[2] = 0x12;
    packet[3] = 0x10;
    for (index, color) in colors.iter().enumerate() {
        let base = 4 + index * 4;
        packet[base] = (index + 1) as u8;
        packet[base + 1] = color.r;
        packet[base + 2] = color.g;
        packet[base + 3] = color.b;
    }
    packet[16] = 0xff;
    packet
}

fn build_apply_packet() -> [u8; PACKET_LEN] {
    let mut packet = [0u8; PACKET_LEN];
    packet[0] = REPORT_ID;
    packet[1] = 0xff;
    packet[2] = 0x12;
    packet[3] = 0x70;
    packet
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_packet_matches_upstream_protocol() {
        let packet = build_init_packet();
        assert_eq!(&packet[..7], &[0x11, 0xff, 0x0e, 0x50, 0x01, 0x03, 0x07]);
        assert_eq!(packet.len(), 20);
    }

    #[test]
    fn set_device_packet_encodes_three_zones() {
        let packet =
            build_set_device_packet(&[Rgb::new(1, 2, 3), Rgb::new(4, 5, 6), Rgb::new(7, 8, 9)]);
        assert_eq!(
            &packet[..17],
            &[
                0x11, 0xff, 0x12, 0x10, 0x01, 1, 2, 3, 0x02, 4, 5, 6, 0x03, 7, 8, 9, 0xff,
            ]
        );
    }

    #[test]
    fn apply_packet_matches_upstream_protocol() {
        assert_eq!(&build_apply_packet()[..4], &[0x11, 0xff, 0x12, 0x70]);
    }

    #[test]
    fn restore_default_is_dim_white() {
        let config: DeviceConfig = toml::from_str(
            r#"
            id = "mouse"
            type = "g102"
            "#,
        )
        .unwrap();
        assert_eq!(
            configured_restore_color(&config, Rgb::WHITE, 0.30),
            Rgb::new(77, 77, 77)
        );
    }
}
