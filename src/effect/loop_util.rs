//! Shared math for the cross-device loop effects.
//!
//! All loop effects use the same clock rate and the same comet shape so they
//! feel like members of one family. `LOOP_RATE` is loops per second at
//! `speed = 1.0`: one full keyboard -> mouse -> laptop trip every 5.5 s.

use crate::effect::EffectContext;

pub(crate) const LOOP_RATE: f32 = 0.18;

/// Current loop phase in 0..1.
pub(crate) fn phase(ctx: &EffectContext, rate: f32) -> f32 {
    ((ctx.time as f32) * ctx.speed * rate).rem_euclid(1.0)
}

/// Brightness for a forward-moving comet.
///
/// `loop_s` and `phase` are positions on the same 0..1 loop. The head is at
/// `phase`; the tail trails behind it, which means lower loop positions just
/// before the head.
pub(crate) fn comet(loop_s: f32, phase: f32, tail: f32) -> f32 {
    comet_behind((phase - loop_s).rem_euclid(1.0), tail)
}

/// Brightness for a comet when you already know how far behind the head the
/// LED is. A short solid core is followed by a quadratic fade.
pub(crate) fn comet_behind(behind: f32, tail: f32) -> f32 {
    const CORE: f32 = 0.012;
    if behind < CORE {
        return 1.0;
    }
    let fade = (1.0 - (behind - CORE) / tail).clamp(0.0, 1.0);
    fade * fade
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comet_head_is_full_and_far_side_is_dark() {
        assert_eq!(comet(0.0, 0.0, 0.11), 1.0);
        assert_eq!(comet(0.5, 0.0, 0.11), 0.0);
    }

    #[test]
    fn comet_tail_fades_with_distance() {
        let near = comet_behind(0.02, 0.11);
        let far = comet_behind(0.08, 0.11);
        assert!(near > far);
        assert!(near > 0.0);
        assert!(far > 0.0);
    }
}
