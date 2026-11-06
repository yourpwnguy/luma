//! Color primitives shared by every effect and driver.
//!
//! The engine works in 8-bit sRGB because that is what all three devices
//! accept. We do not do a full linear-light pipeline: the effects only need
//! smooth interpolation and brightness scaling, and the extra conversion cost
//! would be paid 30 times a second for no visible gain on these panels.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A single 8-bit sRGB color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parse `#RRGGBB` or `RRGGBB`.
    ///
    /// The leading `#` is optional because both forms show up in config files
    /// and on the command line, and rejecting one of them is just annoying.
    pub fn from_hex(value: &str) -> Result<Self, String> {
        let text = value.strip_prefix('#').unwrap_or(value);
        if text.len() != 6 || !text.is_ascii() {
            return Err(format!("expected #RRGGBB, got {value:?}"));
        }
        let r = u8::from_str_radix(&text[0..2], 16)
            .map_err(|_| format!("invalid red component in {value:?}"))?;
        let g = u8::from_str_radix(&text[2..4], 16)
            .map_err(|_| format!("invalid green component in {value:?}"))?;
        let b = u8::from_str_radix(&text[4..6], 16)
            .map_err(|_| format!("invalid blue component in {value:?}"))?;
        Ok(Self { r, g, b })
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Multiply every channel by `factor` and clamp the result.
    ///
    /// Values outside 0..=1 are clamped instead of wrapping. A wrapping bug
    /// here would turn 255 * 1.1 into 24, which would make effects flicker
    /// black at their peaks. Clamping is the boring, correct behavior.
    pub fn scale(self, factor: f32) -> Self {
        let factor = factor.clamp(0.0, 1.0);
        Self {
            r: scale_channel(self.r, factor),
            g: scale_channel(self.g, factor),
            b: scale_channel(self.b, factor),
        }
    }

    #[allow(dead_code)] // kept as the inverse of from_hsv for future effects and tests
    pub fn to_hsv(self) -> Hsv {
        let r = f32::from(self.r) / 255.0;
        let g = f32::from(self.g) / 255.0;
        let b = f32::from(self.b) / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;

        let hue = if delta == 0.0 {
            0.0
        } else if max == r {
            ((g - b) / delta).rem_euclid(6.0) / 6.0
        } else if max == g {
            ((b - r) / delta + 2.0) / 6.0
        } else {
            ((r - g) / delta + 4.0) / 6.0
        };
        let saturation = if max == 0.0 { 0.0 } else { delta / max };
        Hsv {
            h: hue.rem_euclid(1.0),
            s: saturation,
            v: max,
        }
    }

    pub fn from_hsv(hsv: Hsv) -> Self {
        let h = hsv.h.rem_euclid(1.0) * 6.0;
        let s = hsv.s.clamp(0.0, 1.0);
        let v = hsv.v.clamp(0.0, 1.0);
        let c = v * s;
        let x = c * (1.0 - (h % 2.0 - 1.0).abs());
        let m = v - c;
        let (r, g, b) = match h as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        Self {
            r: to_channel(r + m),
            g: to_channel(g + m),
            b: to_channel(b + m),
        }
    }
}

fn scale_channel(value: u8, factor: f32) -> u8 {
    to_channel(f32::from(value) / 255.0 * factor)
}

fn to_channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Hue in turns (0..1), saturation and value in 0..1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsv {
    pub h: f32,
    pub s: f32,
    pub v: f32,
}

impl FromStr for Rgb {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl Serialize for Rgb {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        assert_eq!(
            Rgb::from_hex("#12abEF").unwrap(),
            Rgb::new(0x12, 0xab, 0xef)
        );
        assert_eq!(Rgb::from_hex("000000").unwrap(), Rgb::BLACK);
        assert_eq!(Rgb::new(1, 2, 3).to_hex(), "#010203");
    }

    #[test]
    fn rejects_bad_hex() {
        assert!(Rgb::from_hex("#12345").is_err());
        assert!(Rgb::from_hex("#gggggg").is_err());
        assert!(Rgb::from_hex("").is_err());
    }

    #[test]
    fn hsv_round_trip_primaries() {
        for color in [
            Rgb::new(255, 0, 0),
            Rgb::new(0, 255, 0),
            Rgb::new(0, 0, 255),
            Rgb::new(255, 255, 255),
            Rgb::BLACK,
        ] {
            assert_eq!(Rgb::from_hsv(color.to_hsv()), color);
        }
    }

    #[test]
    fn scaling_clamps_instead_of_wrapping() {
        assert_eq!(Rgb::new(200, 100, 50).scale(2.0), Rgb::new(200, 100, 50));
        assert_eq!(Rgb::new(200, 100, 50).scale(0.0), Rgb::BLACK);
    }
}
