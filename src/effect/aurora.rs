//! Aurora Field: a full-keyboard nebula that never turns the keys off.
//!
//! Two soft waves are evaluated at different scales. The first uses the local
//! key position, so the pattern has detail across the keyboard and mouse. The
//! second uses the shared loop position, so the glow keeps moving onto the
//! laptop instead of stopping at the edge of the keyboard.
//!
//! The brightness floor is deliberately high. This is the effect to use when
//! you want the whole desk to glow, not a thin comet with a lot of black space
//! around it.

use std::f32::consts::TAU;

use crate::color::{Hsv, Rgb};
use crate::effect::palette::mix;
use crate::effect::{Effect, EffectContext};
use crate::geometry::LedSample;

#[derive(Clone, Copy, Debug, Default)]
pub struct Aurora;

impl Effect for Aurora {
    fn name(&self) -> &'static str {
        "aurora"
    }

    fn color_at(&self, ctx: &EffectContext, led: &LedSample) -> Rgb {
        let time = ctx.time as f32 * ctx.speed;
        let x = led.local.x;
        let y = led.local.y;

        // Wave one is the local nebula. Wave two is the cross-device drift.
        // Taking the maximum keeps both visible without adding them into a
        // washed-out white field.
        let wave = 0.5 + 0.5 * (TAU * (x * 0.9 + y * 0.6 - time * 0.06)).sin();
        let swirl = 0.5 + 0.5 * (TAU * (led.loop_s * 1.7 + time * 0.045 + 0.25)).sin();
        let brightness = 0.55 + 0.45 * wave.max(swirl);

        let hue =
            (0.55 + 0.10 * (time * 0.08).sin() + 0.18 * wave + 0.08 * led.loop_s).rem_euclid(1.0);
        let teal = Rgb::from_hsv(Hsv {
            h: hue,
            s: 0.82,
            v: brightness,
        });
        let violet = Rgb::from_hsv(Hsv {
            h: (hue + 0.42).rem_euclid(1.0),
            s: 0.88,
            v: brightness * 0.85,
        });

        let blend = 0.5 + 0.5 * (TAU * (led.loop_s * 0.7 + time * 0.025)).sin();
        mix(teal, violet, 0.35 + 0.65 * blend)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    fn sample(time: f32, x: f32, y: f32, loop_s: f32) -> Rgb {
        Aurora.color_at(
            &EffectContext::new(f64::from(time), 1.0, Rgb::BLACK, 1.0),
            &LedSample::new(Point::new(x, y), loop_s),
        )
    }

    fn brightness(color: Rgb) -> u16 {
        u16::from(color.r) + u16::from(color.g) + u16::from(color.b)
    }

    #[test]
    fn keeps_a_visible_floor_everywhere() {
        for time in [0.0, 4.2, 19.5] {
            for x in [0.0, 0.25, 0.5, 0.75, 1.0] {
                for y in [0.0, 0.5, 1.0] {
                    let color = sample(time, x, y, 0.45 * x);
                    assert!(
                        brightness(color) > 30,
                        "aurora went dark at t={time} x={x} y={y}: {color:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn field_moves_over_time() {
        assert_ne!(sample(0.0, 0.3, 0.4, 0.135), sample(2.0, 0.3, 0.4, 0.135));
    }

    #[test]
    fn loop_position_changes_the_palette() {
        assert_ne!(sample(0.0, 0.5, 0.5, 0.0), sample(0.0, 0.5, 0.5, 0.5));
    }
}
