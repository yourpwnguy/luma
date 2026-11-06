//! Solid color and the explicit "off" effect.

use crate::color::Rgb;
use crate::effect::{Effect, EffectContext};
use crate::geometry::LedSample;

/// Paints the configured base color everywhere.
#[derive(Clone, Copy, Debug)]
pub struct StaticColor {
    color: Option<Rgb>,
}

impl StaticColor {
    pub const fn new() -> Self {
        Self { color: None }
    }

    /// A static effect that ignores the configured base color and emits black.
    pub const fn off() -> Self {
        Self {
            color: Some(Rgb::BLACK),
        }
    }
}

impl Default for StaticColor {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for StaticColor {
    fn name(&self) -> &'static str {
        if self.color.is_some() {
            "off"
        } else {
            "static"
        }
    }

    fn color_at(&self, ctx: &EffectContext, _led: &LedSample) -> Rgb {
        self.color.unwrap_or(ctx.base)
    }
}
