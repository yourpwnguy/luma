//! Layout geometry used to map one effect onto devices of different shapes.
//!
//! Every LED gets a normalized position: x grows left to right, y grows top to
//! bottom, and both are in 0..=1. The effect engine samples that space once per
//! LED. This is what lets a wave look like a wave on a 61-key keyboard and
//! still produce a sensible color on a three-zone mouse or a single-zone
//! laptop.

/// A normalized point on a device.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// One addressable LED as the effect engine sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct LedSpec {
    pub name: String,
    pub position: Point,
}

impl LedSpec {
    pub fn new(name: impl Into<String>, x: f32, y: f32) -> Self {
        Self {
            name: name.into(),
            position: Point::new(x, y),
        }
    }
}
