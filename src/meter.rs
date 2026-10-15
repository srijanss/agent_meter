//! Meter geometry derived from a window's consumed percentage.

/// Fraction of the bar to fill for a consumed percentage, clamped to 0..=1.
///
/// Unknown (`None`) and non-finite values yield `None` so a missing value is
/// never drawn as an empty bar. The raw percentage is left untouched.
pub fn fill_fraction(used_percent: Option<f64>) -> Option<f64> {
    let used = used_percent.filter(|value| value.is_finite())?;
    Some(used.clamp(0.0, 100.0) / 100.0)
}
