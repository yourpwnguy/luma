//! Small color and noise helpers for the atmospheric effects.
//!
//! The five full-field effects need a few operations the basic `Rgb` type does
//! not have: linear mixing, saturating addition, a boost that can go above
//! 1.0, and a stable per-LED hash. We keep them here instead of growing
//! `color.rs`, because these are effect vocabulary, not device color
//! primitives.
//!
//! The hash matters more than it looks. Stars and circuit traces have to stay
//! attached to the same physical key from frame to frame. Random numbers or
//! time would make them crawl, so we quantize the key position and mix it with
//! an integer hash. The salt keeps two effects from drawing the same pattern.

use crate::color::Rgb;

/// Linear blend between two colors. `t` is clamped to 0..=1.
pub(crate) fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let channel = |start: u8, end: u8| {
        let start = f32::from(start);
        let end = f32::from(end);
        (start + (end - start) * t).round().clamp(0.0, 255.0) as u8
    };
    Rgb::new(channel(a.r, b.r), channel(a.g, b.g), channel(a.b, b.b))
}

/// Channel-wise saturating add.
///
/// The atmospheric effects layer a lit base, stars, traces, and packets on top
/// of each other. Saturation is the right behavior at the top end: the result
/// should approach white, not wrap around to black.
pub(crate) fn add(a: Rgb, b: Rgb) -> Rgb {
    Rgb::new(
        a.r.saturating_add(b.r),
        a.g.saturating_add(b.g),
        a.b.saturating_add(b.b),
    )
}

/// Multiply each channel by `factor` and clamp to 0..=255.
///
/// Unlike `Rgb::scale`, this accepts factors above 1.0. Overlapping glows need
/// that little extra push before the final clamp, otherwise the center of a
/// bloom looks flat compared with its edges.
pub(crate) fn amplify(color: Rgb, factor: f32) -> Rgb {
    let channel = |value: u8| (f32::from(value) * factor).round().clamp(0.0, 255.0) as u8;
    Rgb::new(channel(color.r), channel(color.g), channel(color.b))
}

/// Stable 0..1 hash of a position and a salt.
///
/// `x` and `y` do not have to be normalized to 0..1. The constellation effect
/// deliberately adds a device-dependent offset to keep the mouse and laptop
/// from inheriting the same star at the same local center.
pub(crate) fn hash01(x: f32, y: f32, salt: u32) -> f32 {
    let qx = quantize(x);
    let qy = quantize(y);
    let mut hash = salt
        .wrapping_add(qx.wrapping_mul(0x9E37_79B1))
        .wrapping_add(qy.wrapping_mul(0x85EB_CA77));
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x7FEB_352D);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x846C_A68B);
    hash ^= hash >> 16;
    (hash >> 8) as f32 / 16_777_216.0
}

fn quantize(value: f32) -> u32 {
    // About 4096 steps across a device. That is finer than the normalized
    // spacing between keys, so neighboring keys get different stars without
    // making the hash sensitive to tiny floating-point changes.
    (value * 4095.0).round() as i32 as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_hits_both_ends() {
        assert_eq!(mix(Rgb::BLACK, Rgb::WHITE, 0.0), Rgb::BLACK);
        assert_eq!(mix(Rgb::BLACK, Rgb::WHITE, 1.0), Rgb::WHITE);
        assert_eq!(mix(Rgb::BLACK, Rgb::WHITE, 0.5), Rgb::new(128, 128, 128));
        assert_eq!(mix(Rgb::WHITE, Rgb::BLACK, -1.0), Rgb::WHITE);
        assert_eq!(mix(Rgb::WHITE, Rgb::BLACK, 2.0), Rgb::BLACK);
    }

    #[test]
    fn add_saturates_instead_of_wrapping() {
        assert_eq!(
            add(Rgb::new(200, 10, 0), Rgb::new(100, 20, 5)),
            Rgb::new(255, 30, 5)
        );
    }

    #[test]
    fn amplify_can_boost_and_clamps() {
        assert_eq!(amplify(Rgb::new(100, 200, 10), 2.0), Rgb::new(200, 255, 20));
        assert_eq!(amplify(Rgb::new(100, 100, 100), -1.0), Rgb::BLACK);
    }

    #[test]
    fn hash_is_stable_and_in_range() {
        let first = hash01(0.25, 0.75, 0x1234_5678);
        assert_eq!(first, hash01(0.25, 0.75, 0x1234_5678));
        assert!((0.0..=1.0).contains(&first));
        assert_ne!(first, hash01(0.25, 0.75, 0x1234_5679));
        assert_ne!(first, hash01(0.26, 0.75, 0x1234_5678));
    }
}
