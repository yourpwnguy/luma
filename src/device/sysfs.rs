//! Linux hidraw and sysfs discovery.
//!
//! Both USB devices expose two HID interfaces. Interface 0 is the normal boot
//! keyboard or mouse; interface 1 is the vendor control channel. Writing the
//! wrong node is how tools end up sending vendor commands to the input stack,
//! so we identify nodes by three independent facts: VID/PID from `uevent`,
//! the interface number from the parent USB interface, and the presence of the
//! expected report ID in the HID report descriptor.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// A discovered hidraw node and the sysfs facts used to select it.
#[derive(Clone, Debug)]
pub struct HidrawDevice {
    pub path: PathBuf,
    pub interface: Option<u8>,
    pub report_descriptor: Vec<u8>,
    pub product: Option<String>,
    pub serial: Option<String>,
}

impl HidrawDevice {
    pub fn has_report_id(&self, report_id: u8) -> bool {
        self.report_descriptor
            .windows(2)
            .any(|pair| pair == [0x85, report_id])
    }

    pub fn summary(&self) -> String {
        let interface = self
            .interface
            .map(|n| format!("if{n}"))
            .unwrap_or_else(|| "if?".to_string());
        let product = self.product.as_deref().unwrap_or("unknown");
        let serial = self
            .serial
            .as_deref()
            .map(|value| format!(", serial {value}"))
            .unwrap_or_default();
        format!("{} ({interface}, {product}{serial})", self.path.display())
    }
}

/// Find the single hidraw node matching a USB identity.
///
/// `interface` and `report_id` are optional filters. If more than one node
/// remains, that is an error: silently picking one of two candidates is worse
/// than telling the user to replug the device.
pub fn find_hidraw(
    vendor: u16,
    product: u16,
    interface: Option<u8>,
    report_id: Option<u8>,
) -> Result<HidrawDevice> {
    let candidates = find_all_hidraw(vendor, product)?;
    let filtered: Vec<_> = candidates
        .into_iter()
        .filter(|dev| interface.is_none_or(|n| dev.interface == Some(n)))
        .filter(|dev| report_id.is_none_or(|id| dev.has_report_id(id)))
        .collect();

    match filtered.len() {
        0 => bail!(
            "no hidraw node for {:04x}:{:04x} with interface {:?} and report {:?}; \
             check `lsusb` and the HID permissions section in README.md",
            vendor,
            product,
            interface,
            report_id
        ),
        1 => Ok(filtered.into_iter().next().expect("length checked")),
        _ => {
            let list = filtered
                .iter()
                .map(HidrawDevice::summary)
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "multiple hidraw nodes matched {:04x}:{:04x}: {list}; \
                 replug the device or add a more specific udev rule",
                vendor,
                product
            )
        }
    }
}

/// Enumerate every hidraw node for a USB VID/PID, without any interface filter.
pub fn find_all_hidraw(vendor: u16, product: u16) -> Result<Vec<HidrawDevice>> {
    let mut result = Vec::new();
    let entries = fs::read_dir("/sys/class/hidraw")
        .context("cannot read /sys/class/hidraw; is hidraw enabled in the kernel?")?;

    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("hidraw"))
        })
        .collect();
    paths.sort();

    for sysfs in paths {
        let uevent = match fs::read_to_string(sysfs.join("device/uevent")) {
            Ok(text) => text,
            Err(_) => continue,
        };
        let (bus, vid, pid) = match parse_hid_id(&uevent) {
            Some(ids) => ids,
            None => continue,
        };
        if bus != 3 || vid != vendor || pid != product {
            continue;
        }
        let report_descriptor = fs::read(sysfs.join("device/report_descriptor"))
            .with_context(|| format!("cannot read report descriptor for {}", sysfs.display()))?;
        let dev_path = PathBuf::from("/dev").join(
            sysfs
                .file_name()
                .context("hidraw entry without a file name")?,
        );
        result.push(HidrawDevice {
            path: dev_path,
            interface: read_interface_number(&sysfs),
            report_descriptor,
            product: uevent_value(&uevent, "HID_NAME"),
            serial: uevent_value(&uevent, "HID_UNIQ").filter(|s| !s.is_empty()),
        });
    }
    Ok(result)
}

/// Parse the `HID_ID=0003:0000046D:0000C092` line into (bus, vendor, product).
fn parse_hid_id(uevent: &str) -> Option<(u16, u16, u16)> {
    let value = uevent_value(uevent, "HID_ID")?;
    let mut parts = value.split(':');
    let bus = u16::from_str_radix(parts.next()?, 16).ok()?;
    let vendor = u16::from_str_radix(parts.next()?, 16).ok()?;
    let product = u16::from_str_radix(parts.next()?, 16).ok()?;
    Some((bus, vendor, product))
}

fn uevent_value(uevent: &str, key: &str) -> Option<String> {
    uevent.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name == key).then(|| value.trim().to_string())
    })
}

/// Walk up from the hidraw device to the USB interface directory.
fn read_interface_number(sysfs: &Path) -> Option<u8> {
    let mut current = fs::canonicalize(sysfs.join("device")).ok()?;
    for _ in 0..6 {
        let candidate = current.join("bInterfaceNumber");
        if let Ok(text) = fs::read_to_string(&candidate) {
            return u8::from_str_radix(text.trim(), 16).ok();
        }
        if !current.pop() {
            break;
        }
    }
    None
}

/// The ASUS ACPI LED class device. It is a symlink to the platform device, so
/// both the stable class path and the platform path are accepted.
pub fn find_asus_keyboard_led() -> Result<PathBuf> {
    let candidates = [
        PathBuf::from("/sys/class/leds/asus::kbd_backlight"),
        PathBuf::from("/sys/devices/platform/asus-nb-wmi/leds/asus::kbd_backlight"),
    ];
    for candidate in candidates {
        if candidate.join("kbd_rgb_mode").is_file() {
            return Ok(candidate);
        }
    }
    bail!(
        "ASUS keyboard LED interface not found. This model needs a kernel with \
         asus-wmi TUF RGB support (6.2 or newer) and the asus_nb_wmi module loaded. \
         Check `ls /sys/class/leds/asus::kbd_backlight/`."
    )
}

/// Return a short, actionable permission hint for a path.
pub fn permission_hint(path: &Path) -> String {
    format!(
        "permission denied opening {}; install the udev rules with \
         `sudo scripts/install-permissions.sh` and replug the device",
        path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hid_id() {
        let uevent = "HID_ID=0003:0000258A:00000049\nHID_NAME=BY Tech Gaming Keyboard\n";
        assert_eq!(parse_hid_id(uevent), Some((3, 0x258a, 0x0049)));
        assert_eq!(
            uevent_value(uevent, "HID_NAME").as_deref(),
            Some("BY Tech Gaming Keyboard")
        );
    }

    #[test]
    fn rejects_malformed_hid_id() {
        assert_eq!(parse_hid_id("HID_ID=garbage\n"), None);
        assert_eq!(parse_hid_id("HID_NAME=x\n"), None);
    }

    #[test]
    fn detects_report_id() {
        let descriptor = vec![0x06, 0x00, 0xff, 0x85, 0x08, 0x09, 0x01];
        let dev = HidrawDevice {
            path: "/dev/hidraw0".into(),
            interface: Some(1),
            report_descriptor: descriptor,
            product: None,
            serial: None,
        };
        assert!(dev.has_report_id(0x08));
        assert!(!dev.has_report_id(0x11));
    }
}
