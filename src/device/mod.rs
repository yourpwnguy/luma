//! Device abstraction.
//!
//! A driver owns one transport, exposes the LEDs it can address, and turns the
//! engine's per-LED colors into the device's own wire format. The trait is
//! deliberately small: connect, send one frame, restore, close. Anything more
//! device-specific belongs inside the driver, not in the engine.

pub mod asus_tuf;
pub mod g102;
pub mod k617;
pub mod k617_layout;
pub mod mock;
pub mod registry;
pub mod sysfs;

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use anyhow::{Context, Result, bail};
use hidapi::{HidApi, HidDevice};

use crate::color::Rgb;
use crate::geometry::LedSpec;

/// One controllable lighting device.
pub trait Device: Send {
    /// Stable config id.
    fn id(&self) -> &str;
    /// Driver name, e.g. `g102`.
    fn kind(&self) -> &'static str;
    /// Human-readable model, shown by `list-devices` and logs.
    fn model(&self) -> &'static str;
    /// LEDs in the exact order `send` expects colors.
    fn layout(&self) -> &[LedSpec];
    /// Open the transport and capture any restore snapshot.
    fn open(&mut self) -> Result<()>;
    /// Push one frame. `colors.len()` must equal `layout().len()`.
    fn send(&mut self, colors: &[Rgb]) -> Result<()>;
    /// Best-effort restore of the startup state or configured default.
    fn restore(&mut self) -> Result<()>;
    /// Release the transport. Must not perform lighting changes.
    fn close(&mut self);
    /// Whether the transport is currently open.
    fn is_open(&self) -> bool;
}

pub(crate) fn ensure_frame_len(colors: &[Rgb], expected: usize) -> Result<()> {
    if colors.len() != expected {
        bail!(
            "driver got {} colors but its layout has {expected} LEDs",
            colors.len()
        );
    }
    Ok(())
}

/// Open a hidraw path through hidapi's hidraw backend.
///
/// We resolve the path ourselves and only use hidapi for I/O. That keeps the
/// interface selection logic testable and avoids the libusb backend, which
/// would need to detach the kernel driver and disturb normal typing.
pub(crate) fn open_hidraw(path: &Path) -> Result<HidDevice> {
    let api = HidApi::new().context("cannot initialize hidapi")?;
    let c_path = CString::new(path.as_os_str().as_bytes())
        .with_context(|| format!("invalid hidraw path {}", path.display()))?;
    let device = api.open_path(&c_path).map_err(|err| {
        let hint = if err.to_string().to_ascii_lowercase().contains("permission") {
            sysfs::permission_hint(path)
        } else {
            format!(
                "cannot open {}: {err}; check that the device is still attached",
                path.display()
            )
        };
        anyhow::anyhow!(hint)
    })?;
    device
        .set_blocking_mode(true)
        .context("cannot switch hidraw handle to blocking mode")?;
    Ok(device)
}

/// Drain any pending HID++ reply without blocking the frame loop.
pub(crate) fn drain_input(device: &HidDevice) {
    let mut buffer = [0u8; 64];
    let _ = device.read_timeout(&mut buffer, 0);
}
