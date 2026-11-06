//! Prism Ribbon: a wide rainbow ribbon with a moving specular sheen.
//!
//! The hue comes from the shared loop position, so the ribbon continues from
//! the keyboard onto the mouse and laptop instead of restarting at each
//! device. The weave and the bright highlight use the local position, which
//! gives the keyboard a fabric-like texture that a single flat gradient would
//! not have.
//!
//! The brightness floor is high enough that the ribbon reads as a lit surface,
//! not a moving stripe over black.

use std::f32::consts::TAU;

use crate::color::{Hsv, Rgb};
use crate::effect::palette::mix;
use crate::effect::{Effect, EffectContext};
use crate::geometry::LedSample;

#[derive(Clone, Copy, Debug, Default)]
pub struct Ribbon;

impl Effect for Ribbon {
    fn name(&self) -> &'static str {
        "ribbon"
    }

    fn color_at(&self, ctx: &EffectContext, led: &LedSample) -> Rgb {
        let time = ctx.time as f32 * ctx.speed;
        let x = led.local.x;
        let y = led.local.y;

        let hue = (0.02 + led.loop_s * 0.78 + time * 0.035).rem_euclid(1.0);
        let weave = 0.5 + 0.5 * (TAU * (x * 1.8 + y * 0.7 - time * 0.32)).cos();
        let brightness = 0.42 + 0.58 * weave;
        let color = Rgb::from_hsv(Hsv {
            h: hue,
            s: 0.85,
            v: brightness,
        });

        let highlight = 0.5 + 0.5 * (TAU * (x * 3.0 - y * 0.5 - time * 0.55)).cos();
        mix(color, Rgb::WHITE, 0.18 * highlight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    fn sample(time: f32, x: f32, y: f32, loop_s: f32) -> Rgb {
        Ribbon.color_at(
            &EffectContext::new(f64::from(time), 1.0, Rgb::BLACK, 1.0),
            &LedSample::new(Point::new(x, y), loop_s),
        )
    }

    fn brightness(color: Rgb) -> u16 {
        u16::from(color.r) + u16::from(color.g) + u16::from(color.b)
    }

    #[test]
    fn ribbon_never_goes_black() {
        for time in [0.0, 5.5, 22.0] {
            for x in [0.0, 0.25, 0.5, 0.75, 1.0] {
                for y in [0.0, 0.5, 1.0] {
                    let color = sample(time, x, y, 0.45 * x);
                    assert!(
                        brightness(color) > 40,
                        "ribbon went dark at t={time}: {color:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn hue_follows_the_device_loop() {
        assert_ne!(sample(0.0, 0.5, 0.5, 0.0), sample(0.0, 0.5, 0.5, 0.5));
    }

    #[test]
    fn sheen_moves_over_time() {
        assert_ne!(sample(0.0, 0.3, 0.4, 0.135), sample(1.0, 0.3, 0.4, 0.135));
    }
}
