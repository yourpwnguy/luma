//! Global hue cycle.
//!
//! Every LED gets the same color at the same instant. That makes this the
//! clearest effect for checking synchronization across devices, especially
//! the single-zone laptop panel.

use crate::color::{Hsv, Rgb};
use crate::effect::{Effect, EffectContext};
use crate::geometry::LedSample;

#[derive(Clone, Copy, Debug, Default)]
pub struct Cycle;

impl Effect for Cycle {
    fn name(&self) -> &'static str {
        "cycle"
    }

    fn color_at(&self, ctx: &EffectContext, _led: &LedSample) -> Rgb {
        let hue = ((ctx.time as f32) * ctx.speed * 0.1).rem_euclid(1.0);
        Rgb::from_hsv(Hsv {
            h: hue,
            s: 1.0,
            v: 1.0,
        })
    }
}
