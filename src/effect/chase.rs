//! Three comets evenly spaced around the device loop.
//!
//! With three heads, at least one is always in a device slice even when
//! another is crossing the dark gap. The keyboard shows three moving bands,
//! and the mouse and laptop flash three times per loop.

use crate::color::Rgb;
use crate::effect::loop_util::{LOOP_RATE, comet, phase};
use crate::effect::{Effect, EffectContext};
use crate::geometry::LedSample;

const COMETS: usize = 3;

#[derive(Clone, Copy, Debug, Default)]
pub struct Chase;

impl Effect for Chase {
    fn name(&self) -> &'static str {
        "chase"
    }

    fn color_at(&self, ctx: &EffectContext, led: &LedSample) -> Rgb {
        let base_phase = phase(ctx, LOOP_RATE);
        let mut intensity = 0.0f32;
        for index in 0..COMETS {
            let head = (base_phase + index as f32 / COMETS as f32).rem_euclid(1.0);
            intensity = intensity.max(comet(led.loop_s, head, 0.06));
        }
        ctx.base.scale(intensity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{LedSample, Point};

    fn led(loop_s: f32) -> LedSample {
        LedSample::new(Point::new(0.5, 0.5), loop_s)
    }

    #[test]
    fn three_heads_are_bright_at_time_zero() {
        let effect = Chase;
        let ctx = EffectContext::new(0.0, 1.0, Rgb::WHITE, 1.0);
        for position in [0.0, 1.0 / 3.0, 2.0 / 3.0] {
            assert_eq!(effect.color_at(&ctx, &led(position)), Rgb::WHITE);
        }
        assert_eq!(effect.color_at(&ctx, &led(0.5)), Rgb::BLACK);
    }
}
