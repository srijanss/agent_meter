use agent_meter::meter::fill_fraction;

#[test]
fn clamps_consumed_percent_to_the_bar_range_without_touching_raw_value() {
    assert_eq!(fill_fraction(Some(63.0)), Some(0.63));
    assert_eq!(fill_fraction(Some(0.0)), Some(0.0));
    assert_eq!(fill_fraction(Some(150.0)), Some(1.0));
    assert_eq!(fill_fraction(Some(-5.0)), Some(0.0));
    // Unknown or non-finite is never drawn as zero.
    assert_eq!(fill_fraction(None), None);
    assert_eq!(fill_fraction(Some(f64::NAN)), None);

    let raw = 150.0_f64;
    let _ = fill_fraction(Some(raw));
    assert_eq!(raw, 150.0);
}
