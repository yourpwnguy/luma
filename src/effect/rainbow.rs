//! Spatial rainbow.
//!
//! The hue varies along x, so a per-key keyboard shows a real gradient while
//! the mouse and laptop pick the hue at their own center positions. The whole
//! pattern drifts slowly over time, which keeps all devices in the same
//! moving color field.

use crate::color::{Hsv, Rgb};
use crate::effect::{Effect, EffectContext};
use crate::geometry::LedSample;

#[derive(Clone, Copy, Debug, Default)]
pub struct Rainbow;

impl Effect for Rainbow {
    fn name(&self) -> &'static str {
        "rainbow"
    }

    fn color_at(&self, ctx: &EffectContext, led: &LedSample) -> Rgb {
        let hue = (led.local.x * 0.8 + (ctx.time as f32) * ctx.speed * 0.08).rem_euclid(1.0);
        Rgb::from_hsv(Hsv {
            h: hue,
            s: 1.0,
            v: 1.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    #[test]
    fn hue_changes_with_position() {
        let effect = Rainbow;
        let ctx = EffectContext::new(0.0, 1.0, Rgb::BLACK, 1.0);
        let left = effect.color_at(&ctx, &LedSample::new(Point::new(0.0, 0.5), 0.0));
        let right = effect.color_at(&ctx, &LedSample::new(Point::new(0.5, 0.5), 0.0));
        assert_ne!(left, right);
    }
}
