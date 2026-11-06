//! The central effect renderer.
//!
//! One `Engine` instance is shared by every device in a run. For each frame it
//! is given a time in seconds and a device layout, and returns one color per
//! LED in that layout's order. The effect itself never knows which device it is
//! drawing for; capability differences are expressed only through the layout's
//! LED positions.

use anyhow::Result;

use crate::color::Rgb;
use crate::config::EffectSettings;
use crate::effect::{Effect, EffectContext, build};
use crate::geometry::LedSpec;

pub struct Engine {
    effect: Box<dyn Effect>,
    speed: f32,
    base: Rgb,
    wave_cycles: f32,
    brightness: f32,
}

impl Engine {
    pub fn new(settings: &EffectSettings) -> Result<Self> {
        Ok(Self {
            effect: build(&settings.name)?,
            speed: settings.speed,
            base: settings.color,
            wave_cycles: settings.wave_cycles,
            brightness: settings.brightness,
        })
    }

    pub fn name(&self) -> &'static str {
        self.effect.name()
    }

    /// Render one device's frame at `time` seconds.
    ///
    /// Brightness is applied here, after the effect's own envelope, so a
    /// breathing effect at 50% still breathes between 0% and 50% instead of
    /// being clipped.
    pub fn render(&self, time: f64, layout: &[LedSpec]) -> Vec<Rgb> {
        let ctx = EffectContext::new(time, self.speed, self.base, self.wave_cycles);
        layout
            .iter()
            .map(|led| {
                self.effect
                    .color_at(&ctx, led.position)
                    .scale(self.brightness)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(name: &str) -> EffectSettings {
        EffectSettings {
            name: name.to_string(),
            color: Rgb::new(200, 100, 50),
            speed: 1.0,
            brightness: 0.5,
            wave_cycles: 2.0,
        }
    }

    #[test]
    fn applies_brightness_after_the_effect() {
        let engine = Engine::new(&settings("static")).unwrap();
        let layout = vec![LedSpec::new("only", 0.5, 0.5)];
        assert_eq!(engine.render(0.0, &layout), vec![Rgb::new(100, 50, 25)]);
    }

    #[test]
    fn renders_one_color_per_led() {
        let engine = Engine::new(&settings("wave")).unwrap();
        let layout = vec![
            LedSpec::new("a", 0.0, 0.0),
            LedSpec::new("b", 0.25, 0.0),
            LedSpec::new("c", 0.75, 0.0),
        ];
        let frame = engine.render(0.25, &layout);
        assert_eq!(frame.len(), 3);
        assert_ne!(frame[0], frame[1]);
    }
}
