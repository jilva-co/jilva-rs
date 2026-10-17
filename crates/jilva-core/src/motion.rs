//! Motion: easing curves and time-driven transitions
//! (`docs/RFC/0007-motion-and-blending.md`).
//!
//! Everything here is a pure function of an **injected** time value, a
//! [`Duration`] measured from any epoch the caller chooses (RFC-2 §4: `Tick(now)`
//! carries plain data, never a clock). There is no global clock, no timer, no
//! thread, and no async: tests drive time by hand and get bit-identical
//! frames, and an idle app that is not animating costs nothing.
//!
//! ```text
//! Tick(now) ─► transition.value_at(now) ─► View ─► paint ─► diff (only changed cells)
//! ```
//!
//! Reduced motion (`TerminalCapabilities::reduced_motion`) is honoured by
//! [`Transition::value_at_reduced`], which jumps straight to the end value.

use crate::output::ResolvedColor;
use std::time::Duration;

/// A timing curve mapping linear progress `0..=1` to eased progress.
///
/// # Example
///
/// ```rust
/// use jilva_core::Easing;
///
/// assert_eq!(Easing::Linear.apply(0.25), 0.25);
/// assert_eq!(Easing::EaseInOut.apply(0.0), 0.0);
/// assert_eq!(Easing::EaseInOut.apply(1.0), 1.0);
/// assert!(Easing::EaseOut.apply(0.5) > 0.5); // fast start, soft landing
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Easing {
    /// Constant speed.
    Linear,
    /// Starts slow, ends fast (cubic).
    EaseIn,
    /// Starts fast, ends slow (cubic). The default for UI entrances.
    #[default]
    EaseOut,
    /// Slow at both ends (cubic).
    EaseInOut,
    /// Overshoots the target by roughly 10 % and settles back. Overshoot
    /// clamps for colours; it is visible on numbers and positions.
    EaseOutBack,
}

impl Easing {
    /// Maps `t` (clamped to `0..=1`) through the curve. `0` maps to `0` and
    /// `1` maps to `1` for every curve; only [`Easing::EaseOutBack`] leaves
    /// `0..=1` in between.
    #[must_use]
    pub fn apply(self, t: f32) -> f32 {
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t * t,
            Self::EaseOut => {
                let inverse = 1.0 - t;
                1.0 - inverse * inverse * inverse
            }
            Self::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let shifted = -2.0 * t + 2.0;
                    1.0 - shifted * shifted * shifted / 2.0
                }
            }
            Self::EaseOutBack => {
                const OVERSHOOT: f32 = 1.701_58;
                let shifted = t - 1.0;
                1.0 + (OVERSHOOT + 1.0) * shifted * shifted * shifted
                    + OVERSHOOT * shifted * shifted
            }
        }
    }
}

/// A value that can be interpolated.
///
/// `t` is eased progress and is **not** clamped: [`Easing::EaseOutBack`] can
/// pass values above `1`. Implementations keep their result inside the type's
/// range (colour channels saturate).
pub trait Lerp: Sized {
    /// Returns the value `t` of the way from `self` to `to`.
    #[must_use]
    fn lerp(&self, to: &Self, t: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(&self, to: &Self, t: f32) -> Self {
        self + (to - self) * t
    }
}

impl Lerp for i32 {
    /// Interpolates and rounds to the nearest cell.
    fn lerp(&self, to: &Self, t: f32) -> Self {
        (*self as f32 + (*to as f32 - *self as f32) * t).round() as Self
    }
}

impl Lerp for (u8, u8, u8) {
    /// Interpolates each channel, saturating at `0` and `255`.
    fn lerp(&self, to: &Self, t: f32) -> Self {
        let channel = |from: u8, to: u8| {
            (f32::from(from) + (f32::from(to) - f32::from(from)) * t)
                .round()
                .clamp(0.0, 255.0) as u8
        };
        (
            channel(self.0, to.0),
            channel(self.1, to.1),
            channel(self.2, to.2),
        )
    }
}

impl Lerp for ResolvedColor {
    /// Interpolates between two truecolor values. Any other pairing (the
    /// terminal default, a palette index, an ANSI colour) has no meaningful
    /// midpoint without knowing the terminal's palette, so it **steps** from
    /// `self` to `to` at `t >= 0.5`. Fade from or to a theme colour by
    /// resolving it to [`ResolvedColor::Rgb`] first.
    fn lerp(&self, to: &Self, t: f32) -> Self {
        match (self, to) {
            (Self::Rgb(r0, g0, b0), Self::Rgb(r1, g1, b1)) => {
                let (r, g, b) = (*r0, *g0, *b0).lerp(&(*r1, *g1, *b1), t);
                Self::Rgb(r, g, b)
            }
            _ if t >= 0.5 => *to,
            _ => *self,
        }
    }
}

/// A value moving from one state to another over time.
///
/// Immutable and pure: [`Transition::value_at`] is a function of `now` only,
/// so sampling it twice, out of order, or after the end is always safe.
/// Interrupting an animation is [`Transition::retarget`], which starts the new
/// motion from wherever the old one currently is — no jump.
///
/// # Example
///
/// ```rust
/// use std::time::Duration;
/// use jilva_core::{Easing, Transition};
///
/// let fade = Transition::new(0.0_f32, 1.0, Duration::from_millis(100), Duration::from_millis(200), Easing::Linear);
/// assert_eq!(fade.value_at(Duration::from_millis(0)), 0.0);   // before the start
/// assert_eq!(fade.value_at(Duration::from_millis(200)), 0.5); // halfway
/// assert_eq!(fade.value_at(Duration::from_secs(9)), 1.0);     // after the end
/// assert!(fade.is_finished(Duration::from_millis(300)));
///
/// // Interrupt halfway: the new motion starts from 0.5, not from 0.0.
/// let back = fade.retarget(Duration::from_millis(200), 0.0, Duration::from_millis(200));
/// assert_eq!(back.value_at(Duration::from_millis(200)), 0.5);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Transition<T> {
    from: T,
    to: T,
    start: Duration,
    duration: Duration,
    easing: Easing,
}

impl<T: Lerp + Clone> Transition<T> {
    /// Creates a transition that begins at `start` and lasts `duration`.
    #[must_use]
    pub const fn new(from: T, to: T, start: Duration, duration: Duration, easing: Easing) -> Self {
        Self {
            from,
            to,
            start,
            duration,
            easing,
        }
    }

    /// Returns a transition that is already at rest at `value`.
    #[must_use]
    pub fn settled(value: T) -> Self {
        Self::new(
            value.clone(),
            value,
            Duration::ZERO,
            Duration::ZERO,
            Easing::Linear,
        )
    }

    /// Returns the raw (un-eased) progress at `now`, in `0.0..=1.0`. A zero
    /// duration is complete the instant it starts.
    #[must_use]
    pub fn progress(&self, now: Duration) -> f32 {
        if now < self.start {
            return 0.0;
        }
        if self.duration.is_zero() {
            return 1.0;
        }
        let elapsed = now - self.start;
        (elapsed.as_secs_f32() / self.duration.as_secs_f32()).min(1.0)
    }

    /// Returns the value at `now`.
    #[must_use]
    pub fn value_at(&self, now: Duration) -> T {
        let progress = self.progress(now);
        if progress >= 1.0 {
            return self.to.clone();
        }
        self.from.lerp(&self.to, self.easing.apply(progress))
    }

    /// Returns the value at `now`, or the end value immediately when
    /// `reduced_motion` is set (`TerminalCapabilities::reduced_motion`).
    #[must_use]
    pub fn value_at_reduced(&self, now: Duration, reduced_motion: bool) -> T {
        if reduced_motion {
            self.to.clone()
        } else {
            self.value_at(now)
        }
    }

    /// Returns `true` once `now` has reached the end; an app with no running
    /// transition has no reason to request frames.
    #[must_use]
    pub fn is_finished(&self, now: Duration) -> bool {
        self.progress(now) >= 1.0
    }

    /// Returns the end value.
    #[must_use]
    pub const fn target(&self) -> &T {
        &self.to
    }

    /// Redirects the transition at `now`: the new motion starts from the
    /// current value, so interrupting an animation never jumps.
    #[must_use]
    pub fn retarget(&self, now: Duration, to: T, duration: Duration) -> Self {
        Self::new(self.value_at(now), to, now, duration, self.easing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Easing; 5] = [
        Easing::Linear,
        Easing::EaseIn,
        Easing::EaseOut,
        Easing::EaseInOut,
        Easing::EaseOutBack,
    ];

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn every_curve_starts_at_zero_and_ends_at_one() {
        for easing in ALL {
            assert!(near(easing.apply(0.0), 0.0), "{easing:?} start");
            assert!(near(easing.apply(1.0), 1.0), "{easing:?} end");
            // Out-of-range and NaN input never escape.
            assert!(near(easing.apply(-5.0), 0.0));
            assert!(near(easing.apply(9.0), 1.0));
            assert!(near(easing.apply(f32::NAN), 0.0));
        }
    }

    #[test]
    fn curves_have_their_documented_character() {
        assert!(Easing::EaseIn.apply(0.5) < 0.5);
        assert!(Easing::EaseOut.apply(0.5) > 0.5);
        assert!(near(Easing::EaseInOut.apply(0.5), 0.5));
        assert!(Easing::EaseInOut.apply(0.25) < 0.25);
        assert!(Easing::EaseInOut.apply(0.75) > 0.75);
        // Overshoot peaks a bit above 1 before settling.
        let peak = (1..100)
            .map(|i| Easing::EaseOutBack.apply(i as f32 / 100.0))
            .fold(0.0_f32, f32::max);
        assert!(peak > 1.05 && peak < 1.2, "peak {peak}");
        // Monotone curves never decrease.
        for easing in [
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
        ] {
            let mut last = 0.0;
            for i in 0..=100 {
                let value = easing.apply(i as f32 / 100.0);
                assert!(value >= last - 1e-6, "{easing:?} at {i}");
                last = value;
            }
        }
    }

    #[test]
    fn lerp_covers_numbers_cells_and_colours() {
        assert!(near(0.0_f32.lerp(&10.0, 0.25), 2.5));
        assert_eq!(0_i32.lerp(&10, 0.26), 3);
        assert_eq!(10_i32.lerp(&0, 1.0), 0);
        assert_eq!((0, 100, 200).lerp(&(100, 100, 0), 0.5), (50, 100, 100));
        // Overshoot saturates instead of wrapping.
        assert_eq!((200, 200, 200).lerp(&(255, 255, 255), 2.0), (255, 255, 255));
        assert_eq!((50, 50, 50).lerp(&(0, 0, 0), 2.0), (0, 0, 0));
        let a = ResolvedColor::Rgb(0, 0, 0);
        let b = ResolvedColor::Rgb(200, 100, 50);
        assert_eq!(a.lerp(&b, 0.5), ResolvedColor::Rgb(100, 50, 25));
    }

    #[test]
    fn non_truecolor_pairs_step_at_the_midpoint() {
        let from = ResolvedColor::Default;
        let to = ResolvedColor::Palette(40);
        assert_eq!(from.lerp(&to, 0.49), from);
        assert_eq!(from.lerp(&to, 0.5), to);
        let rgb = ResolvedColor::Rgb(1, 2, 3);
        assert_eq!(rgb.lerp(&to, 0.2), rgb);
        assert_eq!(rgb.lerp(&to, 0.9), to);
    }

    #[test]
    fn transition_samples_before_during_and_after() {
        let t = Transition::new(0.0_f32, 100.0, ms(100), ms(400), Easing::Linear);
        assert!(near(t.value_at(ms(0)), 0.0));
        assert!(near(t.value_at(ms(100)), 0.0));
        assert!(near(t.value_at(ms(300)), 50.0));
        assert!(near(t.value_at(ms(500)), 100.0));
        assert!(near(t.value_at(Duration::MAX), 100.0));
        assert!(!t.is_finished(ms(499)));
        assert!(t.is_finished(ms(500)));
        assert!(near(t.progress(ms(300)), 0.5));
        assert_eq!(*t.target(), 100.0);
        // Sampling is pure: order does not matter.
        assert_eq!(t.value_at(ms(300)), t.value_at(ms(300)));
    }

    #[test]
    fn zero_duration_and_settled_transitions_finish_immediately() {
        let instant = Transition::new(1_i32, 9, ms(10), Duration::ZERO, Easing::EaseOut);
        assert_eq!(instant.value_at(ms(9)), 1);
        assert_eq!(instant.value_at(ms(10)), 9);
        let rest = Transition::settled(ResolvedColor::Rgb(1, 2, 3));
        assert_eq!(rest.value_at(ms(0)), ResolvedColor::Rgb(1, 2, 3));
        assert!(rest.is_finished(ms(0)));
    }

    #[test]
    fn reduced_motion_skips_to_the_end_value() {
        let t = Transition::new(0_i32, 40, ms(0), ms(1000), Easing::EaseOut);
        assert_eq!(t.value_at_reduced(ms(10), true), 40);
        assert_eq!(t.value_at_reduced(ms(10), false), t.value_at(ms(10)));
        assert_ne!(t.value_at(ms(10)), 40);
    }

    #[test]
    fn retarget_continues_from_the_current_value_without_a_jump() {
        let out = Transition::new(0.0_f32, 1.0, ms(0), ms(1000), Easing::EaseOut);
        let at = ms(300);
        let seen = out.value_at(at);
        let back = out.retarget(at, 0.0, ms(500));
        assert!(near(back.value_at(at), seen));
        assert!(near(back.value_at(ms(800)), 0.0));
        // Retargeting a finished transition starts from its end value.
        let done = out.retarget(ms(5000), 0.25, ms(100));
        assert!(near(done.value_at(ms(5000)), 1.0));
        assert!(near(done.value_at(ms(5100)), 0.25));
    }

    #[test]
    fn colour_fade_is_deterministic_frame_by_frame() {
        let fade = Transition::new(
            ResolvedColor::Rgb(0, 0, 0),
            ResolvedColor::Rgb(255, 255, 255),
            ms(0),
            ms(100),
            Easing::Linear,
        );
        let frames: Vec<_> = (0..=4).map(|i| fade.value_at(ms(i * 25))).collect();
        assert_eq!(
            frames,
            [
                ResolvedColor::Rgb(0, 0, 0),
                ResolvedColor::Rgb(64, 64, 64),
                ResolvedColor::Rgb(128, 128, 128),
                ResolvedColor::Rgb(191, 191, 191),
                ResolvedColor::Rgb(255, 255, 255),
            ]
        );
    }
}
