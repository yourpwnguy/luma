//! TOML configuration.
//!
//! The config is deliberately strict: unknown top-level keys are errors, not
//! silently ignored. A typo like `brightnesss = 1.0` should fail loudly rather
//! than quietly running at the wrong brightness. Device-specific `options`
//! tables are intentionally open-ended so a new driver can add knobs without
//! changing the config schema.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::color::Rgb;
use crate::stage::StageConfig;

pub const DEFAULT_CONFIG: &str = "config/luma.toml";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub general: General,
    #[serde(default)]
    pub effect: EffectSettings,
    #[serde(default)]
    pub devices: Vec<DeviceConfig>,
    /// Optional loop spans for the cross-device flow effects. Missing entries
    /// fall back to the built-in span for the device kind.
    #[serde(default)]
    pub stage: StageConfig,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct General {
    /// Rendering rate. The K617 has been verified at 30 fps over one 382-byte
    /// feature report per frame.
    #[serde(default = "default_fps")]
    pub fps: u32,
    /// Run for this long, then restore and exit. Omit for "until Ctrl-C".
    #[serde(default)]
    pub duration_secs: Option<f64>,
    /// What to do when a device fails permanently.
    #[serde(default)]
    pub on_error: ErrorPolicy,
    /// Print per-device latency statistics at this interval. Zero disables it.
    #[serde(default = "default_stats_interval")]
    pub stats_interval_secs: f64,
}

impl Default for General {
    fn default() -> Self {
        Self {
            fps: default_fps(),
            duration_secs: None,
            on_error: ErrorPolicy::Continue,
            stats_interval_secs: default_stats_interval(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectSettings {
    #[serde(default = "default_effect")]
    pub name: String,
    #[serde(default = "default_color")]
    pub color: Rgb,
    /// Multiplier. 1.0 is the effect's natural speed.
    #[serde(default = "default_speed")]
    pub speed: f32,
    /// Overall brightness, 0.0..=1.0. Applied after the effect's own envelope.
    #[serde(default = "default_brightness")]
    pub brightness: f32,
    /// Spatial cycles for the wave effect.
    #[serde(default = "default_wave_cycles")]
    pub wave_cycles: f32,
}

impl Default for EffectSettings {
    fn default() -> Self {
        Self {
            name: default_effect(),
            color: default_color(),
            speed: default_speed(),
            brightness: default_brightness(),
            wave_cycles: default_wave_cycles(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceConfig {
    /// Stable name used by `--devices`, logs, and restore errors.
    pub id: String,
    /// Driver name: g102, k617, or asus_tuf.
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// If true, a missing or unopenable device aborts the run. Set false for
    /// machines where a device is only sometimes attached.
    #[serde(default = "default_true")]
    pub required: bool,
    /// Optional per-device cap. The scheduler drops frames to stay under it.
    #[serde(default)]
    pub max_fps: Option<u32>,
    #[serde(default)]
    pub restore: DeviceRestore,
    /// Driver-specific options. Unknown keys here are passed to the driver so
    /// new devices do not need a schema change.
    #[serde(default)]
    pub options: toml::Table,
}

impl DeviceConfig {
    pub fn option_bool(&self, key: &str, default: bool) -> Result<bool> {
        match self.options.get(key) {
            None => Ok(default),
            Some(value) => value
                .as_bool()
                .with_context(|| format!("device {}: options.{key} must be a boolean", self.id)),
        }
    }

    pub fn option_u8(&self, key: &str) -> Result<Option<u8>> {
        match self.options.get(key) {
            None => Ok(None),
            Some(value) => {
                let n = value.as_integer().with_context(|| {
                    format!("device {}: options.{key} must be an integer", self.id)
                })?;
                u8::try_from(n)
                    .map(Some)
                    .with_context(|| format!("device {}: options.{key} is out of range", self.id))
            }
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceRestore {
    #[serde(default)]
    pub policy: RestorePolicy,
    /// Color for `policy = "default"` or as a fallback when no snapshot exists.
    #[serde(default)]
    pub color: Option<Rgb>,
    /// Logical brightness 0.0..=1.0, scaled into the device's own range.
    #[serde(default)]
    pub brightness: Option<f32>,
    /// Hardware brightness level, used by the ASUS TUF driver (0..=3).
    #[serde(default)]
    pub hardware_brightness: Option<u8>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestorePolicy {
    /// Put back the snapshot captured at startup where the protocol allows it.
    #[default]
    Previous,
    /// Apply the configured default color/brightness.
    #[serde(rename = "default")]
    DefaultColor,
    /// Turn the device's LEDs off.
    Off,
    /// Do not touch the device on exit.
    Leave,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorPolicy {
    /// Keep the other devices running and retry the failed one.
    #[default]
    Continue,
    /// Stop the whole run when a device is lost.
    Stop,
}

impl Config {
    pub fn load(path: Option<&Path>) -> Result<(Self, PathBuf)> {
        let path = path
            .map(Path::to_path_buf)
            .unwrap_or_else(resolve_default_path);
        let text = fs::read_to_string(&path)
            .with_context(|| format!("cannot read config {}", path.display()))?;
        let config: Self =
            toml::from_str(&text).with_context(|| format!("invalid config {}", path.display()))?;
        config
            .validate()
            .with_context(|| format!("invalid config {}", path.display()))?;
        Ok((config, path))
    }

    pub fn validate(&self) -> Result<()> {
        if self.general.fps == 0 || self.general.fps > 120 {
            bail!("general.fps must be between 1 and 120");
        }
        if let Some(duration) = self.general.duration_secs {
            if !duration.is_finite() || duration <= 0.0 {
                bail!("general.duration_secs must be a positive number");
            }
        }
        if !self.general.stats_interval_secs.is_finite() || self.general.stats_interval_secs < 0.0 {
            bail!("general.stats_interval_secs must be zero or positive");
        }
        if !self.effect.speed.is_finite() || self.effect.speed < 0.0 {
            bail!("effect.speed must be zero or positive");
        }
        if !(0.0..=1.0).contains(&self.effect.brightness) {
            bail!("effect.brightness must be between 0.0 and 1.0");
        }
        if !self.effect.wave_cycles.is_finite() || self.effect.wave_cycles < 0.0 {
            bail!("effect.wave_cycles must be zero or positive");
        }
        if self.devices.is_empty() {
            bail!("at least one [[devices]] entry is required");
        }

        let mut ids = HashSet::new();
        for device in &self.devices {
            if device.id.trim().is_empty() {
                bail!("every device needs a non-empty id");
            }
            if !ids.insert(device.id.clone()) {
                bail!("duplicate device id {:?}", device.id);
            }
            if device.kind.trim().is_empty() {
                bail!("device {}: type must not be empty", device.id);
            }
            if let Some(max_fps) = device.max_fps {
                if max_fps == 0 || max_fps > 120 {
                    bail!("device {}: max_fps must be between 1 and 120", device.id);
                }
            }
            if let Some(brightness) = device.restore.brightness {
                if !(0.0..=1.0).contains(&brightness) {
                    bail!(
                        "device {}: restore.brightness must be between 0.0 and 1.0",
                        device.id
                    );
                }
            }
        }

        let mut stage_ids = HashSet::new();
        for stage in &self.stage.devices {
            if stage.id.trim().is_empty() {
                bail!("every stage device needs a non-empty id");
            }
            if !stage_ids.insert(stage.id.clone()) {
                bail!("duplicate stage device id {:?}", stage.id);
            }
            if !stage.start.is_finite()
                || !stage.end.is_finite()
                || stage.start < 0.0
                || stage.end > 1.0
                || stage.start >= stage.end
            {
                bail!(
                    "stage device {}: start and end must be finite with \
                     0 <= start < end <= 1",
                    stage.id
                );
            }
            if !self.devices.iter().any(|device| device.id == stage.id) {
                bail!("stage entry for unknown device id {:?}", stage.id);
            }
        }
        Ok(())
    }

    pub fn enabled_devices(&self) -> impl Iterator<Item = &DeviceConfig> {
        self.devices.iter().filter(|device| device.enabled)
    }
}

pub fn resolve_default_path() -> PathBuf {
    let local = PathBuf::from(DEFAULT_CONFIG);
    if local.is_file() {
        return local;
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let candidate = PathBuf::from(xdg).join("luma").join("luma.toml");
        if candidate.is_file() {
            return candidate;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("luma")
            .join("luma.toml");
    }
    local
}

fn default_true() -> bool {
    true
}

fn default_fps() -> u32 {
    30
}

fn default_stats_interval() -> f64 {
    5.0
}

fn default_effect() -> String {
    // A config file without an [effect] section gets the full-field flagship
    // rather than a flat color. The explicit "static" and "off" effects are
    // still available when you want something quiet.
    "aurora".to_string()
}

fn default_color() -> Rgb {
    Rgb::new(0, 170, 255)
}

fn default_speed() -> f32 {
    1.0
}

fn default_brightness() -> f32 {
    0.8
}

fn default_wave_cycles() -> f32 {
    2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"
        [general]
        fps = 30

        [effect]
        name = "wave"
        color = "#00aaff"
        speed = 2.0
        brightness = 0.8
        wave_cycles = 2.0

        [[devices]]
        id = "keyboard"
        type = "k617"

        [[devices]]
        id = "mouse"
        type = "g102"
        max_fps = 30
        [devices.restore]
        policy = "default"
        color = "#ffffff"
        brightness = 0.3

        [[devices]]
        id = "laptop"
        type = "asus_tuf"
        [devices.restore]
        color = "#ffffff"

        [[devices]]
        id = "laptop2"
        type = "asus_tuf"
        enabled = false
    "##;

    #[test]
    fn parses_sample() {
        let config: Config = toml::from_str(SAMPLE).unwrap();
        config.validate().unwrap();
        assert_eq!(config.general.fps, 30);
        assert_eq!(config.effect.name, "wave");
        assert_eq!(config.effect.speed, 2.0);
        assert_eq!(config.effect.color, Rgb::new(0x00, 0xaa, 0xff));
        assert_eq!(config.devices.len(), 4);
        assert_eq!(config.enabled_devices().count(), 3);
        assert_eq!(
            config.devices[1].restore.policy,
            RestorePolicy::DefaultColor
        );
    }

    #[test]
    fn parses_stage_spans() {
        let config: Config = toml::from_str(
            r#"
            [[devices]]
            id = "keyboard"
            type = "k617"
            [[devices]]
            id = "mouse"
            type = "g102"
            [[stage.devices]]
            id = "keyboard"
            start = 0.0
            end = 0.6
            [[stage.devices]]
            id = "mouse"
            start = 0.6
            end = 1.0
            "#,
        )
        .unwrap();
        config.validate().unwrap();
        assert_eq!(config.stage.devices.len(), 2);
        assert_eq!(config.stage.devices[0].start, 0.0);
        assert_eq!(config.stage.devices[0].end, 0.6);
    }

    #[test]
    fn rejects_bad_stage_span() {
        let config: Config = toml::from_str(
            r#"
            [[devices]]
            id = "keyboard"
            type = "k617"
            [[stage.devices]]
            id = "keyboard"
            start = 0.7
            end = 0.7
            "#,
        )
        .unwrap();
        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("stage device"));
    }

    #[test]
    fn rejects_stage_for_unknown_device() {
        let config: Config = toml::from_str(
            r#"
            [[devices]]
            id = "keyboard"
            type = "k617"
            [[stage.devices]]
            id = "typo"
            start = 0.0
            end = 0.5
            "#,
        )
        .unwrap();
        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("unknown device id"));
    }

    #[test]
    fn rejects_duplicate_ids() {
        let config: Config = toml::from_str(
            r#"
            [[devices]]
            id = "same"
            type = "g102"
            [[devices]]
            id = "same"
            type = "g102"
            "#,
        )
        .unwrap();
        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("duplicate"));
    }

    #[test]
    fn rejects_bad_brightness() {
        let config: Config = toml::from_str(
            r#"
            [effect]
            brightness = 1.5
            [[devices]]
            id = "mouse"
            type = "g102"
            "#,
        )
        .unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_unknown_top_level_key() {
        let err = toml::from_str::<Config>(
            r#"
            brightnesss = 1.0
            [[devices]]
            id = "mouse"
            type = "g102"
            "#,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("unknown field") || err.contains("brightnesss"));
    }

    #[test]
    fn device_options_are_typed() {
        let config: Config = toml::from_str(
            r#"
            [[devices]]
            id = "keyboard"
            type = "k617"
            [devices.options]
            allow_missing_snapshot = true
            "#,
        )
        .unwrap();
        assert!(
            config.devices[0]
                .option_bool("allow_missing_snapshot", false)
                .unwrap()
        );
        assert!(config.devices[0].option_bool("nope", true).unwrap());
    }
}
