//! Triad Bloom: three overlapping glows with a heartbeat.
//!
//! Each device slice gets a colored glow: cyan around the keyboard, magenta
//! around the mouse, and amber around the laptop. The glows are wide enough to
//! overlap, so the devices blend instead of looking like three unrelated
//! lamps.
//!
//! The heartbeat is a double pulse, not a pure sine. It reads as lub-dub, then
//! a rest, which is much more alive at low speed. The rest never drops to
//! black: every LED keeps a visible blue-violet floor.

use crate::color::Rgb;
use crate::effect::palette::{add, amplify};
use crate::effect::{Effect, EffectContext};
use crate::geometry::LedSample;

const GLOWS: [(f32, Rgb); 3] = [
    (0.225, Rgb::new(0, 190, 255)),
    (0.575, Rgb::new(255, 40, 160)),
    (0.775, Rgb::new(255, 160, 20)),
];
const FLOOR: Rgb = Rgb::new(16, 16, 32);

#[derive(Clone, Copy, Debug, Default)]
pub struct Bloom;

impl Effect for Bloom {
    fn name(&self) -> &'static str {
        "bloom"
    }

    fn color_at(&self, ctx: &EffectContext, led: &LedSample) -> Rgb {
        let time = ctx.time as f32 * ctx.speed;
        let beat = heartbeat(time * 0.65);
        let response = 0.55 + 0.45 * beat;

        let mut accumulated = Rgb::BLACK;
        let mut total = 0.0;
        for (center, color) in GLOWS {
            let distance = (led.loop_s - center + 0.5).rem_euclid(1.0) - 0.5;
            let glow = (-(distance / 0.30).powi(2)).exp() * response;
            accumulated = add(accumulated, amplify(color, glow));
            total += glow;
        }

        // Normalize the overlapping glows so the middle of the desk does not
        // clip to white. The 1.25 boost restores some punch after normalizing;
        // `amplify` clamps, so a very bright overlap still tops out cleanly.
        let weight = 1.0 / total.max(1.0);
        add(FLOOR, amplify(accumulated, weight * 1.25))
    }
}

/// Double-pulse heartbeat in 0..1.
///
/// One cycle is 1.0. The first pulse is the strong lub at 0.10, the second is a
/// softer dub at 0.30, and the rest of the cycle is quiet. The Gaussian shape
/// is smooth enough to avoid visible stepping at 30 fps.
fn heartbeat(cycle: f32) -> f32 {
    let phase = cycle.rem_euclid(1.0);
    let lub = (-((phase - 0.10) / 0.055).powi(2)).exp();
    let dub = 0.65 * (-((phase - 0.30) / 0.065).powi(2)).exp();
    (lub + dub).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    fn sample(time: f32, loop_s: f32) -> Rgb {
        Bloom.color_at(
            &EffectContext::new(f64::from(time), 1.0, Rgb::BLACK, 1.0),
            &LedSample::new(Point::new(0.5, 0.5), loop_s),
        )
    }

    fn brightness(color: Rgb) -> u16 {
        u16::from(color.r) + u16::from(color.g) + u16::from(color.b)
    }

    #[test]
    fn heartbeat_has_two_pulses_and_a_rest() {
        assert!(heartbeat(0.10) > 0.9);
        assert!(heartbeat(0.30) > 0.4);
        assert!(heartbeat(0.65) < 0.05);
    }

    #[test]
    fn every_device_slice_stays_lit() {
        for time in [0.0, 0.35, 2.0, 11.0] {
            for loop_s in [0.0, 0.225, 0.45, 0.575, 0.70, 0.775, 0.85] {
                let color = sample(time, loop_s);
                assert!(
                    brightness(color) > 40,
                    "bloom went dark at t={time} loop={loop_s}: {color:?}"
                );
            }
        }
    }

    #[test]
    fn the_three_centers_have_different_colors() {
        let keyboard = sample(0.0, 0.225);
        let mouse = sample(0.0, 0.575);
        let laptop = sample(0.0, 0.775);
        assert_ne!(keyboard, mouse);
        assert_ne!(mouse, laptop);
        assert_ne!(keyboard, laptop);
    }

    #[test]
    fn beat_changes_the_same_slice() {
        let peak = sample(0.10 / 0.65, 0.575);
        let rest = sample(0.65 / 0.65, 0.575);
        assert_ne!(peak, rest);
    }
}
