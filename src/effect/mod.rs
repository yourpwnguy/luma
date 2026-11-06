//! The effect registry.
//!
//! Effects are intentionally stateless samplers. The engine calls `color_at`
//! once per LED per frame with a shared time value, so every device sees the
//! same phase. If you later add a stateful effect (fire, sparkle, audio
//! reactive), extend the trait with a `reset`/`tick` pair instead of keeping
//! hidden state per device; one shared instance must stay the source of truth.

mod breathing;
mod cycle;
mod rainbow;
mod static_color;
mod wave;

use anyhow::{Result, bail};

use crate::color::Rgb;
use crate::geometry::Point;

pub use breathing::Breathing;
pub use cycle::Cycle;
pub use rainbow::Rainbow;
pub use static_color::StaticColor;
pub use wave::Wave;

/// Everything an effect needs to know about the current frame.
#[derive(Clone, Copy, Debug)]
pub struct EffectContext {
    /// Seconds since the start of the run. Shared by all devices.
    pub time: f64,
    /// User speed multiplier. 1.0 is the effect's natural speed.
    pub speed: f32,
    /// Base color for color-parameterized effects.
    pub base: Rgb,
    /// Number of spatial wave cycles across a device.
    pub wave_cycles: f32,
}

impl EffectContext {
    pub fn new(time: f64, speed: f32, base: Rgb, wave_cycles: f32) -> Self {
        Self {
            time,
            speed: speed.max(0.0),
            base,
            wave_cycles: wave_cycles.max(0.0),
        }
    }
}

/// A pure function from (time, position) to color.
pub trait Effect: Send + Sync {
    fn name(&self) -> &'static str;
    fn color_at(&self, ctx: &EffectContext, position: Point) -> Rgb;
}

/// Build an effect by CLI/config name. Aliases are deliberately small and
/// obvious; inventing a dozen synonyms makes the help text harder to read.
pub fn build(name: &str) -> Result<Box<dyn Effect>> {
    let effect: Box<dyn Effect> = match name.trim().to_ascii_lowercase().as_str() {
        "static" | "solid" => Box::new(StaticColor::new()),
        "off" => Box::new(StaticColor::off()),
        "breathing" | "breathe" => Box::new(Breathing),
        "rainbow" => Box::new(Rainbow),
        "wave" => Box::new(Wave),
        "cycle" | "spectrum" => Box::new(Cycle),
        other => bail!(
            "unknown effect {other:?}; available: {}",
            NAMES
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    Ok(effect)
}

/// Names and one-line explanations for `luma effects` and the README.
pub const NAMES: &[(&str, &str)] = &[
    ("static", "solid color"),
    ("off", "all LEDs black"),
    (
        "breathing",
        "smooth brightness envelope around the base color",
    ),
    ("rainbow", "spatial rainbow with a slow drift"),
    ("wave", "moving brightness bands of the base color"),
    (
        "cycle",
        "one global hue cycling over time (best on single-zone devices)",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(time: f64) -> EffectContext {
        EffectContext::new(time, 1.0, Rgb::new(255, 0, 0), 2.0)
    }

    #[test]
    fn registry_rejects_unknown_names() {
        let err = match build("disco") {
            Ok(_) => panic!("unknown effect should fail"),
            Err(err) => err.to_string(),
        };
        assert!(err.contains("unknown effect"));
        assert!(err.contains("static"));
    }

    #[test]
    fn aliases_resolve() {
        assert_eq!(build("solid").unwrap().name(), "static");
        assert_eq!(build("spectrum").unwrap().name(), "cycle");
    }

    #[test]
    fn static_is_independent_of_position_and_time() {
        let effect = build("static").unwrap();
        let a = effect.color_at(&ctx(0.0), Point::new(0.0, 0.0));
        let b = effect.color_at(&ctx(99.0), Point::new(1.0, 1.0));
        assert_eq!(a, b);
        assert_eq!(a, Rgb::new(255, 0, 0));
    }

    #[test]
    fn wave_differs_across_space() {
        let effect = build("wave").unwrap();
        let left = effect.color_at(&ctx(0.0), Point::new(0.0, 0.0));
        let quarter = effect.color_at(&ctx(0.0), Point::new(0.25, 0.0));
        assert_ne!(left, quarter);
    }

    #[test]
    fn cycle_is_global() {
        let effect = build("cycle").unwrap();
        let a = effect.color_at(&ctx(1.0), Point::new(0.0, 0.0));
        let b = effect.color_at(&ctx(1.0), Point::new(1.0, 1.0));
        assert_eq!(a, b);
    }
}
