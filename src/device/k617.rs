//! Redragon K617 Fizz / Sinowealth `258a:0049` per-key driver.
//!
//! Two protocols are involved:
//!
//! * Report `0x08` (382 bytes total) streams a complete 126-slot LED frame.
//!   This is the fast path used here: header `08 0a 7a 01`, then 126 RGB
//!   triplets, column-major 16x6 matrix. It has been run at 30 fps on K617
//!   hardware by the fizz-rgb project and by the Sinodragon tooling.
//! * Reports `0x05`/`0x06` read and write the 136-byte configuration block.
//!   We use the read command once at startup so a clean exit can put the
//!   previous firmware effect back. We never send bootloader or flash
//!   commands, and we never touch report `0x09`.
//!
//! The physical layout and slot table live in `k617_layout.rs`. Menu and
//! RCtrl are the two entries that are not hardware-verified.

use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use hidapi::HidDevice;
use log::{debug, info, warn};

use crate::color::Rgb;
use crate::config::{DeviceConfig, RestorePolicy};
use crate::device::sysfs::{find_hidraw, permission_hint};
use crate::device::{Device, ensure_frame_len, open_hidraw};
use crate::geometry::LedSpec;

use super::k617_layout::build_layout;

pub const VENDOR_ID: u16 = 0x258a;
pub const PRODUCT_ID: u16 = 0x0049;
pub const MODEL: &str = "Redragon K617 Fizz";

const REPORT_ID: u8 = 0x08;
const REPORT_LEN: usize = 382;
const LED_SLOTS: usize = 126;
const CONFIG_LEN: usize = 136;
const CONFIG_REPORT_LEN: usize = 1032;
const CONFIG_OFFSET: usize = 8;
const CONFIG_READ_DELAY: Duration = Duration::from_millis(50);

pub struct K617 {
    id: String,
    path: Option<PathBuf>,
    device: Option<HidDevice>,
    layout: Vec<LedSpec>,
    slots: Vec<u8>,
    snapshot: Option<[u8; CONFIG_LEN]>,
    restore_policy: RestorePolicy,
    restore_color: Rgb,
    allow_missing_snapshot: bool,
}

impl K617 {
    pub fn new(config: &DeviceConfig) -> Result<Self> {
        let (layout, slots) = build_layout();
        let restore_color = if config.restore.policy == RestorePolicy::Off {
            Rgb::BLACK
        } else {
            config
                .restore
                .color
                .unwrap_or(Rgb::WHITE)
                .scale(config.restore.brightness.unwrap_or(0.30))
        };
        Ok(Self {
            id: config.id.clone(),
            path: None,
            device: None,
            layout,
            slots,
            snapshot: None,
            restore_policy: config.restore.policy,
            restore_color,
            allow_missing_snapshot: config.option_bool("allow_missing_snapshot", false)?,
        })
    }

    /// Read the 136-byte config block. This is a read-only operation: the
    /// handshake asks the keyboard to hand over its current config.
    fn read_config(&self) -> Result<[u8; CONFIG_LEN]> {
        let device = self
            .device
            .as_ref()
            .context("K617 is not open; call open() first")?;
        device
            .send_feature_report(&[0x05, 0x83, 0xb6, 0x00, 0x00, 0x00])
            .context("K617 config read handshake failed")?;
        thread::sleep(CONFIG_READ_DELAY);

        let mut buffer = vec![0u8; CONFIG_REPORT_LEN];
        buffer[0] = 0x06;
        let read = device
            .get_feature_report(&mut buffer)
            .context("K617 config read failed")?;
        if read < CONFIG_OFFSET + CONFIG_LEN || buffer[0] != 0x06 || buffer[1] != 0x83 {
            bail!(
                "K617 config read returned an unexpected reply (len {read}, head {:02x?})",
                &buffer[..read.min(16)]
            );
        }
        let mut config = [0u8; CONFIG_LEN];
        config.copy_from_slice(&buffer[CONFIG_OFFSET..CONFIG_OFFSET + CONFIG_LEN]);
        Ok(config)
    }

    /// Write the config block back exactly as it was captured.
    fn write_config(&self, config: &[u8; CONFIG_LEN]) -> Result<()> {
        let device = self
            .device
            .as_ref()
            .context("K617 is not open; call open() first")?;
        let mut packet = vec![0u8; CONFIG_REPORT_LEN];
        packet[0] = 0x06;
        packet[1] = 0x03;
        packet[2] = 0xb6;
        packet[CONFIG_OFFSET..CONFIG_OFFSET + CONFIG_LEN].copy_from_slice(config);
        device
            .send_feature_report(&packet)
            .context("K617 config restore failed")?;
        thread::sleep(CONFIG_READ_DELAY);
        Ok(())
    }

    fn send_frame(&mut self, colors: &[Rgb]) -> Result<()> {
        ensure_frame_len(colors, self.layout.len())?;
        let device = self
            .device
            .as_ref()
            .context("K617 is not open; call open() first")?;
        let packet = build_frame_packet(&self.slots, colors)?;
        device
            .send_feature_report(&packet)
            .context("K617 LED frame write failed")?;
        Ok(())
    }
}

impl Device for K617 {
    fn id(&self) -> &str {
        &self.id
    }

    fn kind(&self) -> &'static str {
        "k617"
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

        // The snapshot is captured once. A reconnect must not overwrite it,
        // otherwise a later restore would "restore" the state we left behind
        // during the failed run instead of the user's original config.
        if self.snapshot.is_none() && self.restore_policy == RestorePolicy::Previous {
            match self.read_config() {
                Ok(config) => {
                    let mode = config[13];
                    let color_index = config.get(0x1c + 2 * usize::from(mode)).copied();
                    let speed_bright = config.get(0x1d + 2 * usize::from(mode)).copied();
                    debug!(
                        "{}: snapshot custom={} mode={} start_group={} color_idx={:?} \
                         speed_bright={:?} magic={:02x}{:02x}",
                        self.id,
                        config[12],
                        mode,
                        config[14],
                        color_index,
                        speed_bright,
                        config[0x46],
                        config[0x47]
                    );
                    self.snapshot = Some(config);
                }
                Err(err) if self.allow_missing_snapshot => {
                    warn!(
                        "{}: could not capture config snapshot ({err:#}); \
                         exit restore will use a solid color instead",
                        self.id
                    );
                }
                Err(err) => {
                    return Err(err.context(
                        "refusing to run without a restore snapshot; set \
                         options.allow_missing_snapshot = true to override",
                    ));
                }
            }
        }

        info!(
            "{}: opened {} with {} per-key LEDs at 30 fps",
            self.id,
            found.summary(),
            self.layout.len()
        );
        debug!(
            "{}: snapshot {}",
            self.id,
            if self.snapshot.is_some() {
                "captured"
            } else {
                "not requested"
            }
        );
        Ok(())
    }

    fn send(&mut self, colors: &[Rgb]) -> Result<()> {
        self.send_frame(colors)
    }

    fn restore(&mut self) -> Result<()> {
        if self.restore_policy == RestorePolicy::Leave {
            debug!("{}: restore policy is leave", self.id);
            return Ok(());
        }
        if self.device.is_none() {
            self.open()?;
        }

        if self.restore_policy == RestorePolicy::Previous {
            if let Some(snapshot) = self.snapshot {
                debug!("{}: restoring captured config block", self.id);
                return self.write_config(&snapshot);
            }
            warn!(
                "{}: no config snapshot; restoring a solid fallback color instead",
                self.id
            );
        }

        debug!(
            "{}: restoring direct frame {}",
            self.id,
            self.restore_color.to_hex()
        );
        let colors = vec![self.restore_color; self.layout.len()];
        self.send_frame(&colors)
    }

    fn close(&mut self) {
        self.device = None;
    }

    fn is_open(&self) -> bool {
        self.device.is_some()
    }
}

fn build_frame_packet(slots: &[u8], colors: &[Rgb]) -> Result<Vec<u8>> {
    if slots.len() != colors.len() {
        bail!(
            "K617 slot table has {} entries but frame has {} colors",
            slots.len(),
            colors.len()
        );
    }
    let mut packet = vec![0u8; REPORT_LEN];
    packet[0] = REPORT_ID;
    packet[1] = 0x0a;
    packet[2] = 0x7a;
    packet[3] = 0x01;
    for (color, slot) in colors.iter().zip(slots) {
        let offset = 4 + usize::from(*slot) * 3;
        if offset + 2 >= REPORT_LEN {
            bail!("K617 LED slot {slot} is outside the 126-slot report");
        }
        packet[offset] = color.r;
        packet[offset + 1] = color.g;
        packet[offset + 2] = color.b;
    }
    debug_assert_eq!(packet.len(), LED_SLOTS * 3 + 4);
    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_packet_writes_rgb_at_slot_offsets() {
        let (_, slots) = build_layout();
        let mut colors = vec![Rgb::BLACK; 61];
        colors[0] = Rgb::new(255, 0, 0); // Escape, slot 0
        colors[1] = Rgb::new(0, 255, 0); // 1, slot 7
        let packet = build_frame_packet(&slots, &colors).unwrap();
        assert_eq!(packet.len(), 382);
        assert_eq!(&packet[..4], &[0x08, 0x0a, 0x7a, 0x01]);
        assert_eq!(&packet[4..7], &[255, 0, 0]);
        assert_eq!(&packet[4 + 7 * 3..4 + 7 * 3 + 3], &[0, 255, 0]);
    }

    #[test]
    fn frame_packet_rejects_length_mismatch() {
        let (_, slots) = build_layout();
        assert!(build_frame_packet(&slots, &[Rgb::BLACK; 3]).is_err());
    }

    #[test]
    fn config_packet_layout() {
        let mut config = [0u8; CONFIG_LEN];
        config[0] = 0xaa;
        config[135] = 0xbb;
        let mut packet = vec![0u8; CONFIG_REPORT_LEN];
        packet[0] = 0x06;
        packet[1] = 0x03;
        packet[2] = 0xb6;
        packet[CONFIG_OFFSET..CONFIG_OFFSET + CONFIG_LEN].copy_from_slice(&config);
        assert_eq!(packet.len(), 1032);
        assert_eq!(&packet[..3], &[0x06, 0x03, 0xb6]);
        assert_eq!(packet[8], 0xaa);
        assert_eq!(packet[8 + 135], 0xbb);
    }
}
