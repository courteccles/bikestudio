//! S-UNITS "Done when" checks.

use bike_model::{
    Degrees, LengthUnit, Millimeters, ParseError, commit_angle, commit_length, format_angle,
    format_length, parse_angle, parse_length,
};
use proptest::prelude::*;

const MM: LengthUnit = LengthUnit::Millimeters;
const IN: LengthUnit = LengthUnit::Inches;

/// Lengths a frame design realistically holds, from 0 to 2 m in 0.01 mm steps
/// (S-REF fixtures don't exist yet; this sweep covers every value they could hold
/// at that resolution), plus awkward values.
fn sample_lengths() -> impl Iterator<Item = f64> {
    (0..=200_000).map(|i| i as f64 * 0.01).chain([
        -412.345,
        1e-9,
        0.1 + 0.2,
        25.4 / 3.0,
        1260.0,
        630.0,
        480.0,
    ])
}

#[test]
fn unchanged_field_commits_bit_for_bit() {
    for unit in [MM, IN] {
        for v in sample_lengths() {
            let current = Some(Millimeters(v));
            let shown = format_length(Millimeters(v), unit);
            let committed = commit_length(&shown, &shown, current, unit).unwrap();
            assert_eq!(
                committed.map(|m| m.0.to_bits()),
                Some(v.to_bits()),
                "{v} in {unit:?}"
            );
        }
    }
}

#[test]
fn switching_units_never_changes_the_value() {
    let mut value = Some(Millimeters(1000.0));
    for i in 0..1000 {
        let unit = if i % 2 == 0 { IN } else { MM };
        let shown = format_length(value.unwrap(), unit);
        value = commit_length(&shown, &shown, value, unit).unwrap();
    }
    assert_eq!(value, Some(Millimeters(1000.0)));
}

#[test]
fn typed_inches_read_back_exactly() {
    for thousandths in 0..=80_000 {
        let typed = format!("{}.{:03}", thousandths / 1000, thousandths % 1000);
        let stored = parse_length(&typed, IN, None).unwrap();
        assert_eq!(format_length(stored, IN), typed);
    }
}

#[test]
fn spec_examples_parse_to_25_4_mm() {
    let close = |m: Millimeters| (m.0 - 25.4).abs() < 1e-9;
    assert!(close(parse_length("25.4mm", IN, None).unwrap()));
    assert!(close(parse_length("1in", MM, None).unwrap()));
    assert!(close(parse_length("1\"", MM, None).unwrap()));
    assert!(close(
        parse_length("+0.5", MM, Some(Millimeters(24.9))).unwrap()
    ));
}

#[test]
fn expressions_and_nudges() {
    assert_eq!(parse_length("412.5", MM, None), Ok(Millimeters(412.5)));
    assert_eq!(parse_length("400/2", MM, None), Ok(Millimeters(200.0)));
    assert_eq!(
        parse_length("(10 + 2) * 3", MM, None),
        Ok(Millimeters(36.0))
    );
    assert_eq!(
        parse_length("+2", MM, Some(Millimeters(400.0))),
        Ok(Millimeters(402.0))
    );
    assert_eq!(
        parse_length("-10", MM, Some(Millimeters(400.0))),
        Ok(Millimeters(390.0))
    );
    // Without a current value a sign is just a sign.
    assert_eq!(parse_length("-10", MM, None), Ok(Millimeters(-10.0)));
    // A nudge is in the display unit, or the suffix's unit.
    let nudged = parse_length("+1", IN, Some(Millimeters(100.0))).unwrap();
    assert!((nudged.0 - 125.4).abs() < 1e-9);
    assert_eq!(
        parse_length("+1mm", IN, Some(Millimeters(100.0))),
        Ok(Millimeters(101.0))
    );
}

#[test]
fn travel_stays_in_millimetres() {
    let travel = parse_length("6.7in", bike_model::TRAVEL_UNIT, None).unwrap();
    assert_eq!(format_length(travel, bike_model::TRAVEL_UNIT), "170.2");
}

#[test]
fn errors_are_clear() {
    assert_eq!(parse_length("", MM, None), Err(ParseError::Empty));
    assert_eq!(
        parse_length("10cm", MM, None),
        Err(ParseError::UnknownUnit("cm".into()))
    );
    assert_eq!(parse_length("1 1/8\"", IN, None), Err(ParseError::Fraction));
    assert_eq!(
        parse_length("10/0", MM, None),
        Err(ParseError::DivideByZero)
    );
    assert_eq!(
        parse_length("(1+2", MM, None),
        Err(ParseError::UnclosedParenthesis)
    );
    assert_eq!(parse_length("2*", MM, None), Err(ParseError::Incomplete));
    assert_eq!(
        parse_length(&"(".repeat(100), MM, None),
        Err(ParseError::TooDeep)
    );
    assert_eq!(
        parse_angle("64mm", None),
        Err(ParseError::LengthUnitOnAngle("mm".into()))
    );
    assert_eq!(ParseError::DivideByZero.to_string(), "Can't divide by zero");
}

#[test]
fn formatting() {
    assert_eq!(format_length(Millimeters(630.0), MM), "630.0");
    assert_eq!(format_length(Millimeters(25.4), IN), "1.000");
    assert_eq!(format_length(Millimeters(-0.01), MM), "0.0");
    assert_eq!(format_angle(Degrees(64.0)), "64.0");
    assert_eq!(MM.label(), "mm");
    assert_eq!(IN.label(), "in");
}

#[test]
fn angles() {
    assert_eq!(parse_angle("64", None), Ok(Degrees(64.0)));
    assert_eq!(parse_angle("64.5°", None), Ok(Degrees(64.5)));
    assert_eq!(parse_angle("-0.5", Some(Degrees(64.0))), Ok(Degrees(63.5)));
    let shown = format_angle(Degrees(63.987654));
    assert_eq!(
        commit_angle(&shown, &shown, Some(Degrees(63.987654))),
        Ok(Some(Degrees(63.987654)))
    );
}

#[test]
fn empty_commit_clears_and_unchanged_empty_stays_empty() {
    assert_eq!(
        commit_length("", "630.0", Some(Millimeters(630.0)), MM),
        Ok(None)
    );
    assert_eq!(commit_length("", "", None, MM), Ok(None));
}

proptest! {
    #[test]
    fn arbitrary_input_never_panics(text in ".{0,64}", current in proptest::option::of(-1e6f64..1e6)) {
        for unit in [MM, IN] {
            if let Ok(v) = parse_length(&text, unit, current.map(Millimeters)) {
                prop_assert!(v.0.is_finite());
            }
        }
        let _ = parse_angle(&text, current.map(Degrees));
    }

    #[test]
    fn expression_like_input_never_panics(text in "[0-9.+\\-*/() \"a-z°]{0,48}") {
        let _ = parse_length(&text, MM, Some(Millimeters(1.0)));
        let _ = parse_angle(&text, None);
    }
}
