//! A wide, smooth brightness wave traveling around the device loop.
//!
//! This is the calm member of the flow family. There is no hard comet edge:
//! the keyboard, mouse, and laptop gently brighten and dim in sequence.

use std::f32::consts::TAU;

use crate::color::Rgb;
use crate::effect::loop_util::{LOOP_RATE, phase};
use crate::effect::{Effect, EffectContext};
use crate::geometry::LedSample;

#[derive(Clone, Copy, Debug, Default)]
pub struct Pulse;

impl Effect for Pulse {
    fn name(&self) -> &'static str {
        "pulse"
    }

    fn color_at(&self, ctx: &EffectContext, led: &LedSample) -> Rgb {
        let head = phase(ctx, LOOP_RATE);
        let distance = (led.loop_s - head).rem_euclid(1.0);
        let envelope = 0.5 + 0.5 * (TAU * distance).cos();
        ctx.base.scale(0.08 + 0.92 * envelope)
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
    fn bright_at_the_head_and_dim_half_a_loop_away() {
        let effect = Pulse;
        let ctx = EffectContext::new(0.0, 1.0, Rgb::new(200, 100, 50), 1.0);
        assert_eq!(effect.color_at(&ctx, &led(0.0)), Rgb::new(200, 100, 50));
        let far = effect.color_at(&ctx, &led(0.5));
        assert!(far.r < 30);
    }
}
