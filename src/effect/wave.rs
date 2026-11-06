//! Moving brightness bands of the configured base color.
//!
//! The math is a cosine in the (position, time) plane:
//!
//! ```text
//! phase = x * wave_cycles - t * speed
//! value = 0.12 + 0.88 * (0.5 + 0.5 * cos(2*pi*phase))
//! ```
//!
//! A single-zone device sits at x = 0.5 and therefore pulses at the same rate
//! as the middle of the keyboard, which is the best a one-LED panel can do.

use std::f32::consts::TAU;

use crate::color::Rgb;
use crate::effect::{Effect, EffectContext};
use crate::geometry::LedSample;

#[derive(Clone, Copy, Debug, Default)]
pub struct Wave;

impl Effect for Wave {
    fn name(&self) -> &'static str {
        "wave"
    }

    fn color_at(&self, ctx: &EffectContext, led: &LedSample) -> Rgb {
        let phase = led.local.x * ctx.wave_cycles - (ctx.time as f32) * ctx.speed * 0.5;
        let envelope = 0.5 + 0.5 * (TAU * phase).cos();
        ctx.base.scale(0.12 + 0.88 * envelope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    #[test]
    fn wave_moves_with_time() {
        let effect = Wave;
        let led = LedSample::new(Point::new(0.25, 0.5), 0.0);
        let a = effect.color_at(&EffectContext::new(0.0, 1.0, Rgb::WHITE, 2.0), &led);
        let b = effect.color_at(&EffectContext::new(0.5, 1.0, Rgb::WHITE, 2.0), &led);
        assert_ne!(a, b);
    }
}
