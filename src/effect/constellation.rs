//! Constellation: a lit navy starfield with a shooting star.
//!
//! Every LED sits on a deep-blue wash, so the keyboard never looks switched
//! off between highlights. A stable hash decides which keys are stars. The
//! hash is based on the local position plus a device-dependent loop offset, so
//! the star field does not slide around when the effect animates.
//!
//! The shooting star uses `loop_s`, which means it crosses the keyboard, the
//! mouse, and the laptop as one continuous path. Two faint diagonal lines add
//! a readable constellation shape on the keyboard.

use std::f32::consts::TAU;

use crate::color::Rgb;
use crate::effect::palette::{add, hash01};
use crate::effect::{Effect, EffectContext};
use crate::geometry::LedSample;

const STAR_SALT: u32 = 0xC0_55_7E_11;
const BASE: Rgb = Rgb::new(24, 42, 90);
const LINE: Rgb = Rgb::new(55, 105, 230);

#[derive(Clone, Copy, Debug, Default)]
pub struct Constellation;

impl Effect for Constellation {
    fn name(&self) -> &'static str {
        "constellation"
    }

    fn color_at(&self, ctx: &EffectContext, led: &LedSample) -> Rgb {
        let time = ctx.time as f32 * ctx.speed;
        let wash = 0.85 + 0.15 * (TAU * (led.loop_s * 0.8 + time * 0.04)).sin();
        let mut color = BASE.scale(wash);

        // Including the loop position in the seed keeps the mouse center and
        // the laptop center from getting the exact same star just because they
        // share a local point.
        let star_hash = hash01(led.local.x, led.local.y + led.loop_s * 2.0, STAR_SALT);
        if star_hash > 0.70 {
            let twinkle = 0.5 + 0.5 * (time * 2.2 + star_hash * 30.0).sin();
            let star = (0.45 + 0.55 * twinkle) * (0.6 + 0.4 * star_hash);
            color = add(color, Rgb::new(190, 225, 255).scale(star));
        }

        // Two faint constellation lines. They are position-based, not random,
        // so they stay put and give the keyboard a shape to read.
        if (led.local.y - (0.15 + 0.55 * led.local.x)).abs() < 0.05 {
            color = add(color, LINE.scale(0.20));
        }
        if (led.local.y - 0.72).abs() < 0.04 && led.local.x < 0.72 {
            color = add(color, LINE.scale(0.14));
        }

        let shooting_phase = (time * 0.18).rem_euclid(1.0);
        let distance = (led.loop_s - shooting_phase).rem_euclid(1.0);
        let shooting = (1.0 - distance / 0.12).max(0.0).powi(3);
        add(color, Rgb::WHITE.scale(shooting))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    fn sample(time: f32, x: f32, y: f32, loop_s: f32) -> Rgb {
        Constellation.color_at(
            &EffectContext::new(f64::from(time), 1.0, Rgb::BLACK, 1.0),
            &LedSample::new(Point::new(x, y), loop_s),
        )
    }

    fn brightness(color: Rgb) -> u16 {
        u16::from(color.r) + u16::from(color.g) + u16::from(color.b)
    }

    #[test]
    fn base_wash_never_goes_black() {
        for time in [0.0, 3.0, 14.0] {
            for x in [0.0, 0.4, 1.0] {
                for y in [0.0, 0.5, 1.0] {
                    let color = sample(time, x, y, 0.45 * x);
                    assert!(
                        brightness(color) > 50,
                        "constellation base went dark at t={time}: {color:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn same_star_keeps_its_color_at_the_same_time() {
        let a = sample(1.25, 0.31, 0.27, 0.1395);
        let b = sample(1.25, 0.31, 0.27, 0.1395);
        assert_eq!(a, b);
    }

    #[test]
    fn shooting_star_moves_away_from_the_first_key() {
        let hit = sample(0.0, 0.0, 0.5, 0.0);
        let later = sample(1.0, 0.0, 0.5, 0.0);
        assert!(
            brightness(hit) > brightness(later) + 100,
            "shooting star did not move: hit={hit:?} later={later:?}"
        );
    }
}
