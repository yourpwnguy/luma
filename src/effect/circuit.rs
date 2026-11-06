//! Circuit Flow: a teal circuit board with packets running across it.
//!
//! The base is a dim teal grid, so every key stays lit. Brighter traces are
//! drawn at fixed spatial intervals, and a few hash-selected solder points
//! break up the regularity. Two packets move at different speeds: one follows
//! the shared device loop, and one sweeps across each device locally. The
//! cross-device packet is the part that makes the three devices feel wired
//! together.

use std::f32::consts::TAU;

use crate::color::{Hsv, Rgb};
use crate::effect::palette::{hash01, mix};
use crate::effect::{Effect, EffectContext};
use crate::geometry::LedSample;

const SOLDER_SALT: u32 = 0xC1_7C_01;

#[derive(Clone, Copy, Debug, Default)]
pub struct Circuit;

impl Effect for Circuit {
    fn name(&self) -> &'static str {
        "circuit"
    }

    fn color_at(&self, ctx: &EffectContext, led: &LedSample) -> Rgb {
        let time = ctx.time as f32 * ctx.speed;
        let x = led.local.x;
        let y = led.local.y;

        let grid_x = (x * 61.0).round() as u32;
        let grid_y = (y * 5.0).round() as u32;
        let trace = if grid_x % 5 == 0 || (grid_y % 2 == 0 && grid_x % 3 == 0) {
            0.16
        } else {
            0.0
        };
        let solder = if hash01(x, y, SOLDER_SALT) > 0.93 {
            0.10
        } else {
            0.0
        };
        let row_wave = 0.5 + 0.5 * (TAU * (x * 2.5 - time * 0.7 + y * 0.8)).sin();
        let brightness = (0.30 + trace + solder + 0.18 * row_wave).min(1.0);
        let hue = (0.47 + 0.04 * (time * 0.2 + led.loop_s).sin()).rem_euclid(1.0);
        let mut color = Rgb::from_hsv(Hsv {
            h: hue,
            s: 0.85,
            v: brightness,
        });

        // The main packet travels keyboard -> mouse -> laptop.
        let packet_phase = (time * 0.18).rem_euclid(1.0);
        let distance = (led.loop_s - packet_phase).rem_euclid(1.0);
        let packet = (1.0 - distance / 0.10).max(0.0).powi(3);
        color = mix(color, Rgb::new(180, 255, 255), packet * 0.9);

        // A second, faster packet scans across each device locally. On the
        // mouse this lights the three zones in order; on the laptop it is one
        // pulse when the scan crosses the center.
        let scan_phase = (time * 0.23).rem_euclid(1.0);
        let scan_distance = ((x - scan_phase + 0.5).rem_euclid(1.0) - 0.5).abs();
        let scan = (1.0 - scan_distance / 0.08).max(0.0).powi(3);
        mix(color, Rgb::new(80, 255, 220), scan * 0.35)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    fn sample(time: f32, x: f32, y: f32, loop_s: f32) -> Rgb {
        Circuit.color_at(
            &EffectContext::new(f64::from(time), 1.0, Rgb::BLACK, 1.0),
            &LedSample::new(Point::new(x, y), loop_s),
        )
    }

    fn brightness(color: Rgb) -> u16 {
        u16::from(color.r) + u16::from(color.g) + u16::from(color.b)
    }

    #[test]
    fn base_board_is_visible_everywhere() {
        for time in [0.0, 2.5, 17.0] {
            for x in [0.0, 0.33, 0.66, 1.0] {
                for y in [0.0, 0.5, 1.0] {
                    let color = sample(time, x, y, 0.45 * x);
                    assert!(
                        brightness(color) > 40,
                        "circuit base went dark at t={time}: {color:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn cross_device_packet_head_is_brighter_than_the_far_side() {
        let head = sample(0.0, 0.5, 0.5, 0.0);
        let far = sample(0.0, 0.5, 0.5, 0.5);
        assert!(brightness(head) > brightness(far));
    }

    #[test]
    fn local_scan_moves_away_from_the_left_edge() {
        let start = sample(0.0, 0.0, 0.5, 0.2);
        let later = sample(1.5, 0.0, 0.5, 0.2);
        assert!(brightness(start) > brightness(later));
    }
}
