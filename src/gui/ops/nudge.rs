//! Settings-button attention nudge (no-backend UX).
/// Total nudge window (seconds, egui time).
///
/// Timing follows the common “attention double-pulse” pattern (≈ Animate.css flash
/// cadence, but with soft half-sine envelopes instead of hard on/off).
pub const SETTINGS_NUDGE_SECONDS: f64 = 0.90;

/// First soft pulse: start offset and duration within the nudge window.
pub(crate) const NUDGE_PULSE1_START: f32 = 0.0;
pub(crate) const NUDGE_PULSE1_DUR: f32 = 0.34;
/// Second softer pulse (decayed amplitude — less strobe-y than two equal flashes).
pub(crate) const NUDGE_PULSE2_START: f32 = 0.42;
pub(crate) const NUDGE_PULSE2_DUR: f32 = 0.34;
pub(crate) const NUDGE_PULSE2_GAIN: f32 = 0.55;

/// Steady highlight used when the user prefers reduced motion.
pub(crate) const NUDGE_REDUCED_MOTION_STRENGTH: f32 = 0.55;

/// True when a primary click should pulse the Settings button (no backend, not on allowed controls).
pub fn click_should_nudge_settings(backend_ok: bool, click_on_allowed_control: bool) -> bool {
    !backend_ok && !click_on_allowed_control
}

/// Whether the settings-button nudge animation is still running at `now` (egui time).
pub fn settings_nudge_active(until: Option<f64>, now: f64) -> bool {
    until.is_some_and(|t| now < t)
}

/// Soft highlight strength `0.0..=1.0` for the Settings button during a nudge.
///
/// When `reduced_motion` is true, holds a steady fill for the whole window.
/// Otherwise two half-sine “bell” pulses (smooth fade in/out); second peak weaker.
/// Intensity only — callers must not change widget size or stroke width.
pub fn settings_nudge_highlight(until: Option<f64>, now: f64, reduced_motion: bool) -> f32 {
    let Some(until) = until else {
        return 0.0;
    };
    if now >= until {
        return 0.0;
    }
    let start = until - SETTINGS_NUDGE_SECONDS;
    let elapsed = (now - start) as f32;
    if elapsed < 0.0 {
        return 0.0;
    }
    if reduced_motion {
        return NUDGE_REDUCED_MOTION_STRENGTH;
    }
    let a = half_sine_bell(elapsed, NUDGE_PULSE1_START, NUDGE_PULSE1_DUR);
    let b = NUDGE_PULSE2_GAIN * half_sine_bell(elapsed, NUDGE_PULSE2_START, NUDGE_PULSE2_DUR);
    (a + b).min(1.0)
}

/// Unit half-sine envelope over `[start, start+duration]`: 0 → 1 → 0.
pub(crate) fn half_sine_bell(t: f32, start: f32, duration: f32) -> f32 {
    if duration <= f32::EPSILON {
        return 0.0;
    }
    let u = (t - start) / duration;
    if !(0.0..=1.0).contains(&u) {
        0.0
    } else {
        (u * std::f32::consts::PI).sin()
    }
}
