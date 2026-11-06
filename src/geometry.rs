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

/// One LED with both its local device position and its place on the shared
/// 0..1 device loop.
///
/// Per-device effects use `local`. Cross-device flow effects use `loop_s` so
/// the keyboard, mouse, and laptop are treated as one strip of light rather
/// than three unrelated devices.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LedSample {
    pub local: Point,
    pub loop_s: f32,
}

impl LedSample {
    pub const fn new(local: Point, loop_s: f32) -> Self {
        Self { local, loop_s }
    }
}
