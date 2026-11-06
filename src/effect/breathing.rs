//! Breathing: the base color fades up and down in place.
//!
//! At speed 1.0 one full breath takes two seconds. A small floor keeps the
//! LEDs from going completely black at the bottom of the curve, which tends to
//! look like a dropout rather than an intentional fade.

use std::f32::consts::TAU;

use crate::color::Rgb;
use crate::effect::{Effect, EffectContext};
use crate::geometry::LedSample;

#[derive(Clone, Copy, Debug, Default)]
pub struct Breathing;

impl Effect for Breathing {
    fn name(&self) -> &'static str {
        "breathing"
    }

    fn color_at(&self, ctx: &EffectContext, _led: &LedSample) -> Rgb {
        let phase = (ctx.time as f32) * ctx.speed * 0.5;
        let envelope = 0.5 - 0.5 * (TAU * phase).cos();
        ctx.base.scale(0.05 + 0.95 * envelope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    #[test]
    fn starts_near_black_and_peaks_halfway_through() {
        let effect = Breathing;
        let led = LedSample::new(Point::new(0.5, 0.5), 0.0);
        let dark = effect.color_at(
            &EffectContext::new(0.0, 1.0, Rgb::new(200, 100, 50), 1.0),
            &led,
        );
        let bright = effect.color_at(
            &EffectContext::new(1.0, 1.0, Rgb::new(200, 100, 50), 1.0),
            &led,
        );
        assert!(dark.r <= 15);
        assert_eq!(bright, Rgb::new(200, 100, 50));
    }
}
