//! Rainbow gradient wrapped around the device loop.
//!
//! The keyboard, mouse, and laptop share one hue field. At any instant the
//! keyboard shows the first part of the rainbow, the mouse the next part, and
//! the laptop a single later hue. Drifting the field over time makes the whole
//! setup look like one continuous strip instead of three separate devices.

use crate::color::{Hsv, Rgb};
use crate::effect::{Effect, EffectContext};
use crate::geometry::LedSample;

#[derive(Clone, Copy, Debug, Default)]
pub struct Flow;

impl Effect for Flow {
    fn name(&self) -> &'static str {
        "flow"
    }

    fn color_at(&self, ctx: &EffectContext, led: &LedSample) -> Rgb {
        let hue = (led.loop_s + (ctx.time as f32) * ctx.speed * 0.06).rem_euclid(1.0);
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
    use crate::geometry::{LedSample, Point};

    #[test]
    fn hue_follows_loop_position() {
        let effect = Flow;
        let ctx = EffectContext::new(0.0, 1.0, Rgb::BLACK, 1.0);
        let a = effect.color_at(&ctx, &LedSample::new(Point::new(0.5, 0.5), 0.0));
        let b = effect.color_at(&ctx, &LedSample::new(Point::new(0.5, 0.5), 0.5));
        assert_ne!(a, b);
    }
}
