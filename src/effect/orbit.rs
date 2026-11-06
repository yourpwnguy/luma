//! A single comet with a fading tail moving around the device loop.
//!
//! The comet spends 45 percent of the loop on the keyboard, 25 percent on the
//! mouse, 15 percent on the laptop, and 15 percent in the dark gap that makes
//! the restart visible. The base color is used for the comet.

use crate::color::Rgb;
use crate::effect::loop_util::{LOOP_RATE, comet, phase};
use crate::effect::{Effect, EffectContext};
use crate::geometry::LedSample;

#[derive(Clone, Copy, Debug, Default)]
pub struct Orbit;

impl Effect for Orbit {
    fn name(&self) -> &'static str {
        "orbit"
    }

    fn color_at(&self, ctx: &EffectContext, led: &LedSample) -> Rgb {
        let head = phase(ctx, LOOP_RATE);
        ctx.base.scale(comet(led.loop_s, head, 0.11))
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
    fn head_is_bright_and_opposite_side_is_dark() {
        let effect = Orbit;
        let ctx = EffectContext::new(0.0, 1.0, Rgb::new(0, 255, 0), 1.0);
        assert_eq!(effect.color_at(&ctx, &led(0.0)), Rgb::new(0, 255, 0));
        assert_eq!(effect.color_at(&ctx, &led(0.5)), Rgb::BLACK);
    }

    #[test]
    fn head_moves_with_time() {
        let effect = Orbit;
        let start = effect.color_at(&EffectContext::new(0.0, 1.0, Rgb::WHITE, 1.0), &led(0.18));
        let later = effect.color_at(&EffectContext::new(1.0, 1.0, Rgb::WHITE, 1.0), &led(0.18));
        assert_ne!(start, later);
    }
}
