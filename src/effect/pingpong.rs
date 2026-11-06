//! A comet that travels forward, then reverses and comes back.
//!
//! This feels like tracing the triangle with a flashlight: keyboard to mouse
//! to laptop, pause, then laptop back to mouse to keyboard. The tail stays
//! behind the direction of travel in both legs.

use crate::color::Rgb;
use crate::effect::loop_util::{LOOP_RATE, comet_behind, phase};
use crate::effect::{Effect, EffectContext};
use crate::geometry::LedSample;

#[derive(Clone, Copy, Debug, Default)]
pub struct PingPong;

impl Effect for PingPong {
    fn name(&self) -> &'static str {
        "pingpong"
    }

    fn color_at(&self, ctx: &EffectContext, led: &LedSample) -> Rgb {
        let cycle = phase(ctx, LOOP_RATE);
        let forward = cycle < 0.5;
        let head = if forward {
            cycle * 2.0
        } else {
            2.0 - cycle * 2.0
        };
        let behind = if forward {
            (head - led.loop_s).rem_euclid(1.0)
        } else {
            (led.loop_s - head).rem_euclid(1.0)
        };
        ctx.base.scale(comet_behind(behind, 0.10))
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
    fn tail_flips_side_when_the_comet_reverses() {
        let effect = PingPong;
        // cycle 0.25 -> head 0.5, moving forward; cycle 0.75 -> head 0.5,
        // moving backward.
        let forward = EffectContext::new(0.25 / LOOP_RATE as f64, 1.0, Rgb::WHITE, 1.0);
        let backward = EffectContext::new(0.75 / LOOP_RATE as f64, 1.0, Rgb::WHITE, 1.0);
        assert_ne!(effect.color_at(&forward, &led(0.45)), Rgb::BLACK);
        assert_eq!(effect.color_at(&forward, &led(0.55)), Rgb::BLACK);
        assert_eq!(effect.color_at(&backward, &led(0.45)), Rgb::BLACK);
        assert_ne!(effect.color_at(&backward, &led(0.55)), Rgb::BLACK);
    }
}
