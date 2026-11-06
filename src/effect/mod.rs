//! The effect registry.
//!
//! Effects are intentionally stateless samplers. The engine calls `color_at`
//! once per LED per frame with a shared time value, so every device sees the
//! same phase. If you later add a stateful effect (fire, sparkle, audio
//! reactive), extend the trait with a `reset`/`tick` pair instead of keeping
//! hidden state per device; one shared instance must stay the source of truth.
//!
//! Per-device effects read `led.local`. Cross-device flow effects read
//! `led.loop_s`, the LED's position on the shared 0..1 device loop described
//! in `stage.rs`. The atmospheric effects read both: local detail for stars,
//! traces, and weave, and the loop position for the cross-device motion.

mod aurora;
mod bloom;
mod breathing;
mod chase;
mod circuit;
mod constellation;
mod cycle;
mod flow;
mod loop_util;
mod orbit;
mod palette;
mod pingpong;
mod pulse;
mod rainbow;
mod ribbon;
mod static_color;
mod wave;

use anyhow::{Result, bail};

use crate::color::Rgb;
use crate::geometry::LedSample;

pub use aurora::Aurora;
pub use bloom::Bloom;
pub use breathing::Breathing;
pub use chase::Chase;
pub use circuit::Circuit;
pub use constellation::Constellation;
pub use cycle::Cycle;
pub use flow::Flow;
pub use orbit::Orbit;
pub use pingpong::PingPong;
pub use pulse::Pulse;
pub use rainbow::Rainbow;
pub use ribbon::Ribbon;
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

/// A pure function from one LED sample to a color.
pub trait Effect: Send + Sync {
    fn name(&self) -> &'static str;
    fn color_at(&self, ctx: &EffectContext, led: &LedSample) -> Rgb;
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
        "flow" => Box::new(Flow),
        "orbit" => Box::new(Orbit),
        "chase" => Box::new(Chase),
        "pulse" => Box::new(Pulse),
        "pingpong" => Box::new(PingPong),
        "aurora" | "aurora-field" => Box::new(Aurora),
        "constellation" | "stars" => Box::new(Constellation),
        "circuit" | "circuit-flow" => Box::new(Circuit),
        "ribbon" | "prism" | "prism-ribbon" => Box::new(Ribbon),
        "bloom" | "triad" | "triad-bloom" => Box::new(Bloom),
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
    (
        "flow",
        "rainbow gradient that travels around the keyboard -> mouse -> laptop loop",
    ),
    ("orbit", "one comet sweeping keyboard -> mouse -> laptop"),
    ("chase", "three comets chasing around the device loop"),
    (
        "pulse",
        "wide brightness wave traveling around the device loop",
    ),
    (
        "pingpong",
        "comet bouncing forward and backward around the device loop",
    ),
    (
        "aurora",
        "full-field teal and violet nebula; every key stays lit",
    ),
    (
        "constellation",
        "lit navy starfield with twinkling stars and a shooting star",
    ),
    (
        "circuit",
        "teal circuit board with traces and moving data packets",
    ),
    ("ribbon", "wide prism rainbow ribbon with a moving sheen"),
    ("bloom", "three overlapping glows pulsing like a heartbeat"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    fn ctx(time: f64) -> EffectContext {
        EffectContext::new(time, 1.0, Rgb::new(255, 0, 0), 2.0)
    }

    fn led(x: f32, y: f32) -> LedSample {
        LedSample::new(Point::new(x, y), 0.0)
    }

    fn loop_led(loop_s: f32) -> LedSample {
        LedSample::new(Point::new(0.5, 0.5), loop_s)
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
    fn loop_effects_are_registered() {
        for name in ["flow", "orbit", "chase", "pulse", "pingpong"] {
            assert_eq!(build(name).unwrap().name(), name);
        }
    }

    #[test]
    fn atmospheric_effects_are_registered() {
        for name in ["aurora", "constellation", "circuit", "ribbon", "bloom"] {
            assert_eq!(build(name).unwrap().name(), name);
        }
    }

    #[test]
    fn atmospheric_effects_keep_every_led_lit() {
        // This is the requirement that drove these effects: no key should
        // spend most of the loop sitting at black. Check every channel, not
        // just the sum, so a very blue effect cannot hide a dead red channel.
        let samples = [
            ("keyboard-left", 0.0, 0.5, 0.0),
            ("keyboard-center", 0.5, 0.5, 0.225),
            ("keyboard-right", 1.0, 0.5, 0.45),
            ("mouse-left", 0.0, 0.5, 0.45),
            ("mouse-center", 0.5, 0.5, 0.575),
            ("mouse-right", 1.0, 0.5, 0.70),
            ("laptop", 0.5, 0.5, 0.775),
        ];
        for name in ["aurora", "constellation", "circuit", "ribbon", "bloom"] {
            let effect = build(name).unwrap();
            for time in [0.0, 0.37, 2.5, 11.3] {
                let ctx = EffectContext::new(time, 1.0, Rgb::BLACK, 1.0);
                for (label, x, y, loop_s) in samples {
                    let color = effect.color_at(&ctx, &LedSample::new(Point::new(x, y), loop_s));
                    assert!(
                        color.r > 0 && color.g > 0 && color.b > 0,
                        "{name} turned off {label} at t={time}: {color:?}"
                    );
                }
                for row in 0..5 {
                    for column in 0..12 {
                        let x = column as f32 / 11.0;
                        let y = (row as f32 + 0.5) / 5.0;
                        let color =
                            effect.color_at(&ctx, &LedSample::new(Point::new(x, y), 0.45 * x));
                        assert!(
                            color.r > 0 && color.g > 0 && color.b > 0,
                            "{name} turned off a keyboard key at t={time} x={x} y={y}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn atmospheric_effects_are_not_dim() {
        // "Every channel above zero" is necessary but not sufficient. The
        // original complaint was that most keys felt off, so check that the
        // average key is actually visible and that the effect has real peaks.
        fn luminance(color: Rgb) -> f32 {
            0.2126 * f32::from(color.r) + 0.7152 * f32::from(color.g) + 0.0722 * f32::from(color.b)
        }

        for name in ["aurora", "constellation", "circuit", "ribbon", "bloom"] {
            let effect = build(name).unwrap();
            let mut sum = 0.0;
            let mut count = 0u32;
            let mut peak: f32 = 0.0;
            for time in [0.0, 1.3, 5.7, 13.1] {
                let ctx = EffectContext::new(time, 1.0, Rgb::BLACK, 1.0);
                for row in 0..5 {
                    for column in 0..12 {
                        let x = column as f32 / 11.0;
                        let y = (row as f32 + 0.5) / 5.0;
                        let color =
                            effect.color_at(&ctx, &LedSample::new(Point::new(x, y), 0.45 * x));
                        let value = luminance(color);
                        sum += value;
                        count += 1;
                        peak = peak.max(value);
                    }
                }
            }
            let average = sum / count as f32;
            assert!(
                average > 60.0,
                "{name} average luminance is only {average:.1}"
            );
            assert!(peak > 140.0, "{name} has no bright peak: {peak:.1}");
        }
    }

    #[test]
    fn atmospheric_effects_use_their_signature_palettes() {
        // These five have a fixed look. --color is for the older effects, and
        // silently tinting a reviewed palette would make the previews lie.
        let red = EffectContext::new(1.0, 1.0, Rgb::new(255, 0, 0), 1.0);
        let blue = EffectContext::new(1.0, 1.0, Rgb::new(0, 0, 255), 1.0);
        let led = LedSample::new(Point::new(0.4, 0.6), 0.18);
        for name in ["aurora", "constellation", "circuit", "ribbon", "bloom"] {
            let effect = build(name).unwrap();
            assert_eq!(
                effect.color_at(&red, &led),
                effect.color_at(&blue, &led),
                "{name} changed with the base color"
            );
        }
    }

    #[test]
    fn static_is_independent_of_position_and_time() {
        let effect = build("static").unwrap();
        let a = effect.color_at(&ctx(0.0), &led(0.0, 0.0));
        let b = effect.color_at(&ctx(99.0), &led(1.0, 1.0));
        assert_eq!(a, b);
        assert_eq!(a, Rgb::new(255, 0, 0));
    }

    #[test]
    fn wave_differs_across_space() {
        let effect = build("wave").unwrap();
        let left = effect.color_at(&ctx(0.0), &led(0.0, 0.0));
        let quarter = effect.color_at(&ctx(0.0), &led(0.25, 0.0));
        assert_ne!(left, quarter);
    }

    #[test]
    fn cycle_is_global() {
        let effect = build("cycle").unwrap();
        let a = effect.color_at(&ctx(1.0), &led(0.0, 0.0));
        let b = effect.color_at(&ctx(1.0), &led(1.0, 1.0));
        assert_eq!(a, b);
    }

    #[test]
    fn orbit_head_is_bright_and_the_far_side_is_dark() {
        let effect = build("orbit").unwrap();
        assert_eq!(
            effect.color_at(&ctx(0.0), &loop_led(0.0)),
            Rgb::new(255, 0, 0)
        );
        assert_eq!(effect.color_at(&ctx(0.0), &loop_led(0.5)), Rgb::BLACK);
    }
}
