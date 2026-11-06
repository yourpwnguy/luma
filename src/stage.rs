//! The shared device loop.
//!
//! Flow effects need one dimension that every device can share. We use a
//! normalized 0..1 loop and give each device a slice of it. Sorted by start
//! value, the default layout is:
//!
//! ```text
//! keyboard 0.00 .. 0.45
//! mouse    0.45 .. 0.70
//! laptop   0.70 .. 0.85
//! gap      0.85 .. 1.00   (no LEDs, so the loop visibly restarts)
//! ```
//!
//! A 61-key keyboard maps its left-to-right position into its slice, so the
//! comet sweeps the keys. The three mouse zones map into the mouse slice. The
//! single laptop zone sits at the center of its slice and pulses when the
//! comet arrives. Change the spans in `[[stage.devices]]` to reverse the order
//! or make one device linger longer.

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::geometry::LedSpec;

/// The optional `[stage]` config section.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageConfig {
    #[serde(default)]
    pub devices: Vec<StageDevice>,
}

/// One device's slice of the 0..1 loop.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageDevice {
    pub id: String,
    pub start: f32,
    pub end: f32,
}

#[derive(Clone, Copy, Debug)]
struct Span {
    start: f32,
    end: f32,
}

/// Resolved loop spans, keyed by config device id.
#[derive(Clone, Debug)]
pub struct Stage {
    spans: HashMap<String, Span>,
}

/// One LED's position on the shared loop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StageSample {
    pub loop_s: f32,
}

impl Stage {
    pub fn new(config: &StageConfig) -> Self {
        let spans = config
            .devices
            .iter()
            .map(|device| {
                (
                    device.id.clone(),
                    Span {
                        start: device.start,
                        end: device.end,
                    },
                )
            })
            .collect();
        Self { spans }
    }

    /// Check that every selected device has either an explicit span or a
    /// built-in default for its kind. Called before devices are opened, so a
    /// config mistake cannot leave a half-open session behind.
    pub fn validate_for<'a>(
        &self,
        devices: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<()> {
        for (id, kind) in devices {
            if self.span_for(id, kind).is_none() {
                bail!(
                    "no loop span for device {id:?} (type {kind:?}); add a \
                     [[stage.devices]] entry with id, start, and end"
                );
            }
        }
        Ok(())
    }

    /// Map a device layout into its loop slice.
    ///
    /// A one-LED device uses the middle of its slice. Multi-LED devices use
    /// their local left-to-right position, which is what makes the keyboard
    /// and the mouse sweep instead of flashing as one block.
    pub fn samples_for(
        &self,
        id: &str,
        kind: &str,
        layout: &[LedSpec],
    ) -> Result<Vec<StageSample>> {
        let span = self.span_for(id, kind).with_context(|| {
            format!(
                "no loop span for device {id:?} (type {kind:?}); add a \
                 [[stage.devices]] entry"
            )
        })?;
        let width = span.end - span.start;
        let single = layout.len() == 1;
        Ok(layout
            .iter()
            .map(|led| {
                let t = if single {
                    0.5
                } else {
                    led.position.x.clamp(0.0, 1.0)
                };
                StageSample {
                    loop_s: span.start + width * t,
                }
            })
            .collect())
    }

    fn span_for(&self, id: &str, kind: &str) -> Option<Span> {
        self.spans.get(id).copied().or_else(|| default_span(kind))
    }
}

fn default_span(kind: &str) -> Option<Span> {
    match kind.trim().to_ascii_lowercase().as_str() {
        "k617" => Some(Span {
            start: 0.00,
            end: 0.45,
        }),
        "g102" => Some(Span {
            start: 0.45,
            end: 0.70,
        }),
        "asus_tuf" | "asus-tuf" | "tuf" => Some(Span {
            start: 0.70,
            end: 0.85,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::LedSpec;

    fn layout(positions: &[f32]) -> Vec<LedSpec> {
        positions
            .iter()
            .enumerate()
            .map(|(index, x)| LedSpec::new(format!("led{index}"), *x, 0.5))
            .collect()
    }

    #[test]
    fn default_spans_cover_the_three_devices() {
        let stage = Stage::new(&StageConfig::default());
        let keyboard = stage
            .samples_for("keyboard", "k617", &layout(&[0.0, 0.5, 1.0]))
            .unwrap();
        assert_eq!(keyboard[0].loop_s, 0.0);
        assert!((keyboard[1].loop_s - 0.225).abs() < 1e-6);
        assert!((keyboard[2].loop_s - 0.45).abs() < 1e-6);

        let mouse = stage
            .samples_for("mouse", "g102", &layout(&[0.0, 0.5, 1.0]))
            .unwrap();
        assert!((mouse[0].loop_s - 0.45).abs() < 1e-6);
        assert!((mouse[1].loop_s - 0.575).abs() < 1e-6);
        assert!((mouse[2].loop_s - 0.70).abs() < 1e-6);

        let laptop = stage
            .samples_for("laptop", "asus_tuf", &layout(&[0.5]))
            .unwrap();
        assert!((laptop[0].loop_s - 0.775).abs() < 1e-6);
    }

    #[test]
    fn explicit_span_overrides_the_kind_default() {
        let config = StageConfig {
            devices: vec![StageDevice {
                id: "keyboard".to_string(),
                start: 0.10,
                end: 0.20,
            }],
        };
        let stage = Stage::new(&config);
        let samples = stage
            .samples_for("keyboard", "k617", &layout(&[0.0, 1.0]))
            .unwrap();
        assert!((samples[0].loop_s - 0.10).abs() < 1e-6);
        assert!((samples[1].loop_s - 0.20).abs() < 1e-6);
    }

    #[test]
    fn unknown_kind_without_a_span_is_an_error() {
        let stage = Stage::new(&StageConfig::default());
        let err = stage
            .samples_for("other", "custom", &layout(&[0.5]))
            .unwrap_err()
            .to_string();
        assert!(err.contains("no loop span"));
    }

    #[test]
    fn validate_for_catches_missing_placements() {
        let stage = Stage::new(&StageConfig::default());
        assert!(
            stage
                .validate_for([("keyboard", "k617"), ("mouse", "g102")])
                .is_ok()
        );
        assert!(stage.validate_for([("other", "custom")]).is_err());
    }
}
