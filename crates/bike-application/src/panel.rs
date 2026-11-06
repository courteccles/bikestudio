//! The Specification panel's view model (S-SHELL H4–H6): what each field shows,
//! and the command a typed value or lock click becomes. Plain Rust, so it's
//! tested without a window.

use bike_model::{
    ANGLE_LABEL, Degrees, GeometryVariable as V, LengthUnit, Material, Millimeters, Parameter,
    ParseError, PedalSize, TRAVEL_UNIT, TubeKind, ValueSource, commit_angle, commit_length,
    format_angle, format_length,
};

use crate::document::{Command, Data, Document, End};

/// One editable value in the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Field {
    Travel(End),
    Geometry(V),
    SeatTubeLength,
    TopTubeJunction,
    DownTubeJunction,
    SeatTubeOverhang,
    /// Minimum bend radius of a bent seat tube (S-TUBES B2).
    SeatTubeBendRadius,
    /// Seatpost insertion depth, read only (S-TUBES B2).
    SeatTubeInsertion,
    TubeDiameter(TubeKind),
    TubeWall(TubeKind),
    CrankLength,
    PedalLength,
    PedalThickness,
    ForkAxleToCrown,
    ForkOffset,
    /// See [`crate::scene::standover`]; pinned, it moves the seat tube
    /// junction (S-EDIT-CENTERS E2).
    Standover,
}

impl Field {
    /// The letter drawn on the side view and shown next to the field. A–O
    /// follow the geometry drawing the designer works from (C and P aren't
    /// used); Q and R are extra.
    pub fn letter(self) -> Option<char> {
        Some(match self {
            Field::Geometry(V::Reach) => 'A',
            Field::Geometry(V::Stack) => 'B',
            Field::Geometry(V::HeadTubeAngle) => 'D',
            Field::Geometry(V::ActualSeatTubeAngle) => 'E',
            Field::Geometry(V::EffectiveSeatTubeAngle) => 'F',
            Field::Geometry(V::FrontCentre) => 'G',
            Field::Geometry(V::RearCentre) => 'H',
            Field::Geometry(V::Wheelbase) => 'I',
            Field::Geometry(V::BottomBracketHeight) => 'J',
            Field::Standover => 'K',
            Field::SeatTubeLength => 'L',
            Field::ForkOffset => 'M',
            Field::Geometry(V::HeadTubeLength) => 'N',
            Field::ForkAxleToCrown => 'O',
            Field::Geometry(V::BottomBracketDrop) => 'Q',
            Field::Geometry(V::ChainstayLength) => 'R',
            Field::SeatTubeOverhang => 'S',
            _ => return None,
        })
    }

    /// Geometry rows drawn in one box together share a group: head tube,
    /// seat tube, bottom bracket, fork and cranks.
    pub fn group(self) -> Option<u8> {
        Some(match self {
            Field::Geometry(V::HeadTubeAngle | V::HeadTubeLength) => 0,
            Field::Geometry(V::EffectiveSeatTubeAngle | V::ActualSeatTubeAngle)
            | Field::SeatTubeLength
            | Field::SeatTubeOverhang
            | Field::SeatTubeInsertion => 1,
            Field::Geometry(V::BottomBracketDrop | V::BottomBracketHeight) => 2,
            Field::ForkOffset | Field::ForkAxleToCrown => 3,
            Field::CrankLength | Field::PedalLength | Field::PedalThickness => 4,
            _ => return None,
        })
    }
}

/// The header drawn at the top of a group's box.
pub fn group_title(group: u8) -> &'static str {
    match group {
        0 => "Head tube",
        1 => "Saddle",
        2 => "Bottom bracket",
        3 => "Fork",
        _ => "Cranks",
    }
}

/// Where a Geometry row sits in the panel.
fn geometry_rank(field: Field) -> usize {
    const ORDER: [Field; 18] = [
        Field::Geometry(V::Reach),
        Field::Geometry(V::Stack),
        Field::Geometry(V::HeadTubeAngle),
        Field::Geometry(V::HeadTubeLength),
        Field::Geometry(V::EffectiveSeatTubeAngle),
        Field::Geometry(V::ActualSeatTubeAngle),
        Field::SeatTubeLength,
        Field::SeatTubeOverhang,
        Field::SeatTubeInsertion,
        Field::Standover,
        Field::Geometry(V::FrontCentre),
        Field::Geometry(V::RearCentre),
        Field::Geometry(V::ChainstayLength),
        Field::Geometry(V::Wheelbase),
        Field::Geometry(V::BottomBracketDrop),
        Field::Geometry(V::BottomBracketHeight),
        Field::ForkOffset,
        Field::ForkAxleToCrown,
    ];
    ORDER
        .iter()
        .position(|f| *f == field)
        .unwrap_or(ORDER.len())
}

/// The panel's sections, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Travel,
    Geometry,
    Tubes,
}

impl Section {
    pub fn label(self) -> &'static str {
        match self {
            Section::Travel => "Travel",
            Section::Geometry => "Geometry",
            Section::Tubes => "Tubes",
        }
    }
}

/// What a field shows.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldView {
    pub field: Field,
    pub section: Section,
    pub label: String,
    /// The value as displayed; empty when there is none.
    pub text: String,
    pub unit: &'static str,
    /// `Some` for fields with a lock: whether it's pinned.
    pub locked: Option<bool>,
    /// Free and defaulted values are dimmed (H4).
    pub dimmed: bool,
    /// "default" next to a value filled from the defaults.
    pub is_default: bool,
    /// The side view's letter for this value.
    pub letter: Option<char>,
    /// Calculated only: shown, not typed into.
    pub read_only: bool,
}

impl Field {
    fn is_angle(self) -> bool {
        matches!(self, Field::Geometry(v) if v.is_angle())
    }

    fn unit(self, display: LengthUnit) -> LengthUnit {
        match self {
            Field::Travel(_) => TRAVEL_UNIT,
            _ => display,
        }
    }
}

/// The text an Up (`up`) or Down arrow gives a field showing `text`.
/// Angles step 0.1°, millimetres 1 and inches 0.1; `big` (Shift) steps ten
/// times as far. `None` when the field can't be typed into or `text` isn't
/// a number.
pub fn step(
    view: &FieldView,
    text: &str,
    up: bool,
    big: bool,
    display: LengthUnit,
) -> Option<String> {
    if view.read_only {
        return None;
    }
    let shown = if text.trim().is_empty() {
        &view.text
    } else {
        text
    };
    let value: f64 = shown.trim().parse().ok()?;
    let unit = view.field.unit(display);
    let mut amount = if view.field.is_angle() {
        0.1
    } else {
        match unit {
            LengthUnit::Millimeters => 1.0,
            LengthUnit::Inches => 0.1,
        }
    };
    if big {
        amount *= 10.0;
    }
    let next = if up { value + amount } else { value - amount };
    Some(if view.field.is_angle() {
        format_angle(Degrees(next))
    } else {
        format_length(Millimeters::from_unit(next, unit), unit)
    })
}

/// Every field, in panel order, as it shows now. The document must be solved.
pub fn fields(doc: &Document, data: &Data, display: LengthUnit) -> Vec<FieldView> {
    let mut out = Vec::new();
    let p = &doc.project;
    let geometry = doc.solved.geometry.as_ref();
    let tubes = doc.solved.tubes.as_ref();
    let length = |v: Option<f64>, unit: LengthUnit| {
        v.map_or_else(String::new, |v| format_length(Millimeters(v), unit))
    };

    for end in [End::Front, End::Rear] {
        let travel = match end {
            End::Front => p.front_travel,
            End::Rear => p.rear_travel,
        };
        out.push(FieldView {
            field: Field::Travel(end),
            section: Section::Travel,
            label: end_label(end).into(),
            text: format_length(travel.unwrap_or(Millimeters(0.0)), TRAVEL_UNIT),
            unit: TRAVEL_UNIT.label(),
            locked: None,
            dimmed: travel.is_none(),
            is_default: false,
            letter: None,
            read_only: false,
        });
    }

    for v in V::ALL {
        let pinned = p.geometry.pinned(v);
        let source = geometry.and_then(|g| g.sources.get(&v)).copied();
        let value = pinned.or_else(|| geometry.map(|g| g.static_state.value(v)));
        let text = match value {
            Some(x) if v.is_angle() => format_angle(Degrees(x)),
            Some(x) => format_length(Millimeters(x), display),
            None => String::new(),
        };
        out.push(FieldView {
            field: Field::Geometry(v),
            section: Section::Geometry,
            label: v.label().into(),
            text,
            unit: if v.is_angle() {
                ANGLE_LABEL
            } else {
                display.label()
            },
            locked: Some(pinned.is_some()),
            dimmed: pinned.is_none(),
            is_default: pinned.is_none() && source == Some(ValueSource::Default),
            letter: None,
            read_only: false,
        });
    }
    for (field, label, set, solved) in [
        (
            Field::ForkAxleToCrown,
            "Axle-to-crown",
            p.fork.axle_to_crown,
            geometry.map(|g| g.fork_axle_to_crown),
        ),
        (
            Field::ForkOffset,
            "Fork offset",
            p.fork.offset,
            geometry.map(|g| g.fork_offset),
        ),
    ] {
        out.push(optional_length(
            field,
            Section::Geometry,
            label,
            set,
            solved,
            display,
        ));
    }
    let standover_pinned = p.tubes.standover.pinned();
    let standover = standover_pinned.or_else(|| crate::scene::standover(&doc.solved));
    out.push(FieldView {
        field: Field::Standover,
        section: Section::Geometry,
        label: "Standover".into(),
        text: length(standover, display),
        unit: display.label(),
        locked: Some(standover_pinned.is_some()),
        dimmed: standover_pinned.is_none(),
        is_default: false,
        letter: None,
        read_only: false,
    });
    let mut overhang = optional_length(
        Field::SeatTubeOverhang,
        Section::Geometry,
        "Mast",
        p.tubes.junctions.seat_tube_overhang,
        tubes.map(|t| t.seat_tube_overhang),
        display,
    );
    // Solved from a pinned standover: calculated, not a default.
    if tubes.is_some_and(|t| t.seat_tube_overhang_source == ValueSource::Solved) {
        overhang.is_default = false;
    }
    out.push(overhang);
    let mut insertion = optional_length(
        Field::SeatTubeInsertion,
        Section::Geometry,
        "Insertion depth",
        None,
        tubes.map(|t| t.seat_tube_insertion),
        display,
    );
    insertion.read_only = true;
    insertion.is_default = false;
    out.push(insertion);
    let seat_pinned = p.tubes.seat_tube_length.pinned();
    let seat = seat_pinned.or_else(|| tubes.map(|t| t.seat_tube_length));
    out.push(FieldView {
        field: Field::SeatTubeLength,
        section: Section::Geometry,
        label: "Seat tube length".into(),
        text: length(seat, display),
        unit: display.label(),
        locked: Some(seat_pinned.is_some()),
        dimmed: seat_pinned.is_none(),
        is_default: seat_pinned.is_none(),
        letter: None,
        read_only: false,
    });
    // Geometry rows run in the order the designer reads a geometry chart.
    let start = out
        .iter()
        .position(|v| v.section == Section::Geometry)
        .unwrap_or(out.len());
    out[start..].sort_by_key(|v| geometry_rank(v.field));

    for kind in TubeKind::ALL {
        let tube = tubes.and_then(|t| t.tube(kind));
        let section = p.tubes.sections.get(&kind).copied().unwrap_or_default();
        out.push(optional_length(
            Field::TubeDiameter(kind),
            Section::Tubes,
            &format!("{} diameter", kind.label()),
            section.outside_diameter,
            tube.map(|t| t.outside_diameter),
            display,
        ));
        out.push(optional_length(
            Field::TubeWall(kind),
            Section::Tubes,
            &format!("{} wall", kind.label()),
            section.wall,
            tube.map(|t| t.wall),
            display,
        ));
    }

    out.push(optional_length(
        Field::SeatTubeBendRadius,
        Section::Tubes,
        "Seat tube bend radius",
        p.tubes.seat_tube_bend_radius,
        tubes.map(|t| t.seat_tube_bend_radius),
        display,
    ));

    let pedal = p.crank.pedal;
    out.push(optional_length(
        Field::CrankLength,
        Section::Geometry,
        "Crank length",
        p.crank.crank_length,
        geometry.map(|g| g.crank_length),
        display,
    ));
    out.push(optional_length(
        Field::PedalLength,
        Section::Geometry,
        "Pedal length",
        pedal.map(|p| p.length),
        geometry.map(|g| g.pedal_length),
        display,
    ));
    out.push(optional_length(
        Field::PedalThickness,
        Section::Geometry,
        "Pedal thickness",
        pedal.map(|p| p.thickness),
        geometry.map(|g| g.pedal_thickness),
        display,
    ));
    let _ = data;
    for view in &mut out {
        view.letter = view.field.letter();
    }
    out
}

/// A length with no lock: set by the designer, or the default shown dimmed.
fn optional_length(
    field: Field,
    section: Section,
    label: &str,
    set: Option<Millimeters>,
    solved: Option<f64>,
    display: LengthUnit,
) -> FieldView {
    let value = set.map(|m| m.0).or(solved);
    FieldView {
        field,
        section,
        label: label.into(),
        text: value.map_or_else(String::new, |v| format_length(Millimeters(v), display)),
        unit: display.label(),
        locked: None,
        dimmed: set.is_none(),
        is_default: set.is_none() && value.is_some(),
        letter: None,
        read_only: false,
    }
}

pub fn end_label(end: End) -> &'static str {
    match end {
        End::Front => "Front",
        End::Rear => "Rear",
    }
}

/// The command a committed field becomes, or `None` if the text is unchanged
/// (the no-drift rule, S-UNITS U7). Typing into a free geometry field pins it
/// (H4). Clearing a field changes nothing (it shows its value again); a
/// value is freed with its lock.
pub fn commit(
    doc: &Document,
    view: &FieldView,
    text: &str,
    display: LengthUnit,
) -> Result<Option<Command>, ParseError> {
    // Unchanged, or cleared by accident: the field keeps its value.
    if text.trim() == view.text.trim() || text.trim().is_empty() {
        return Ok(None);
    }
    let field = view.field;
    if field.is_angle() {
        let Field::Geometry(v) = field else {
            unreachable!()
        };
        let current = shown_value(doc, field).map(Degrees);
        return Ok(Some(match commit_angle(text, &view.text, current)? {
            Some(d) => Command::PinGeometry(v, d.0),
            None => Command::FreeGeometry(v),
        }));
    }
    let unit = field.unit(display);
    // BB drop can be negative, so a leading sign there is the value's sign,
    // not a nudge to the current value (U7): "-30" is −30 mm.
    let signed = matches!(field, Field::Geometry(V::BottomBracketDrop));
    let current = shown_value(doc, field).map(Millimeters).filter(|_| !signed);
    let value = commit_length(text, &view.text, current, unit)?;
    let p = &doc.project;
    let mut tubes = p.tubes.clone();
    let mut crank = p.crank;
    Ok(Some(match field {
        Field::Travel(end) => Command::SetTravel(end, value.filter(|m| m.0 != 0.0)),
        Field::Geometry(v) => match value {
            Some(m) => Command::PinGeometry(v, m.0),
            None => Command::FreeGeometry(v),
        },
        Field::SeatTubeLength => {
            tubes.seat_tube_length = value.map_or(Parameter::Free, |m| Parameter::Pinned(m.0));
            Command::SetTubes(tubes)
        }
        Field::TopTubeJunction => {
            tubes.junctions.top_tube_below_head_tube_top = value;
            Command::SetTubes(tubes)
        }
        Field::DownTubeJunction => {
            tubes.junctions.down_tube_above_head_tube_bottom = value;
            Command::SetTubes(tubes)
        }
        Field::SeatTubeOverhang => {
            tubes.junctions.seat_tube_overhang = value;
            Command::SetTubes(tubes)
        }
        Field::SeatTubeBendRadius => {
            tubes.seat_tube_bend_radius = value;
            Command::SetTubes(tubes)
        }
        Field::SeatTubeInsertion => return Ok(None),
        Field::TubeDiameter(kind) | Field::TubeWall(kind) => {
            let section = tubes.sections.entry(kind).or_default();
            if matches!(field, Field::TubeDiameter(_)) {
                section.outside_diameter = value;
            } else {
                section.wall = value;
            }
            if section.outside_diameter.is_none() && section.wall.is_none() {
                tubes.sections.remove(&kind);
            }
            Command::SetTubes(tubes)
        }
        Field::ForkAxleToCrown | Field::ForkOffset => {
            let mut fork = p.fork;
            if field == Field::ForkOffset {
                fork.offset = value;
            } else {
                fork.axle_to_crown = value;
            }
            Command::SetFork(fork)
        }
        Field::Standover => {
            tubes.standover = value.map_or(Parameter::Free, |m| Parameter::Pinned(m.0));
            Command::SetTubes(tubes)
        }
        Field::CrankLength => {
            crank.crank_length = value;
            Command::SetCrank(crank)
        }
        Field::PedalLength | Field::PedalThickness => {
            let shown = doc.solved.geometry.as_ref();
            let mut pedal = crank.pedal.unwrap_or(PedalSize {
                length: Millimeters(shown.map_or(100.0, |g| g.pedal_length)),
                thickness: Millimeters(shown.map_or(20.0, |g| g.pedal_thickness)),
            });
            match (field, value) {
                (Field::PedalLength, Some(m)) => pedal.length = m,
                (Field::PedalThickness, Some(m)) => pedal.thickness = m,
                _ => {}
            }
            crank.pedal = value.map(|_| pedal);
            Command::SetCrank(crank)
        }
    }))
}

/// The lock's click: pinned → free; free → pinned at the value shown (H4).
pub fn toggle_lock(doc: &Document, field: Field) -> Option<Command> {
    match field {
        Field::Geometry(v) => Some(match doc.project.geometry.pinned(v) {
            Some(_) => Command::FreeGeometry(v),
            None => Command::PinGeometry(v, shown_value(doc, field)?),
        }),
        Field::SeatTubeLength => {
            let mut tubes = doc.project.tubes.clone();
            tubes.seat_tube_length = match tubes.seat_tube_length {
                Parameter::Pinned(_) => Parameter::Free,
                Parameter::Free => Parameter::Pinned(shown_value(doc, field)?),
            };
            Some(Command::SetTubes(tubes))
        }
        Field::Standover => {
            let mut tubes = doc.project.tubes.clone();
            tubes.standover = match tubes.standover {
                Parameter::Pinned(_) => Parameter::Free,
                Parameter::Free => Parameter::Pinned(shown_value(doc, field)?),
            };
            Some(Command::SetTubes(tubes))
        }
        _ => None,
    }
}

/// Why `text` isn't allowed in a field, or `None`. Only values that parse
/// are checked here; a parse error is reported when the field commits. The
/// panel blocks these and outlines the field.
pub fn not_allowed(
    doc: &Document,
    view: &FieldView,
    text: &str,
    display: LengthUnit,
) -> Option<String> {
    match commit(doc, view, text, display) {
        Ok(Some(Command::PinGeometry(v, value))) => {
            doc.seat_angle_limit(v, value).map(|(limit, rule)| {
                format!("{} {rule} ({}°).", view.label, format_angle(Degrees(limit)))
            })
        }
        _ => None,
    }
}

/// The full-precision value a field shows.
fn shown_value(doc: &Document, field: Field) -> Option<f64> {
    let p = &doc.project;
    let g = doc.solved.geometry.as_ref();
    let t = doc.solved.tubes.as_ref();
    match field {
        Field::Travel(End::Front) => p.front_travel.map(|m| m.0),
        Field::Travel(End::Rear) => p.rear_travel.map(|m| m.0),
        Field::Geometry(v) => p
            .geometry
            .pinned(v)
            .or_else(|| g.map(|g| g.static_state.value(v))),
        Field::SeatTubeLength => p
            .tubes
            .seat_tube_length
            .pinned()
            .or_else(|| t.map(|t| t.seat_tube_length)),
        Field::TopTubeJunction => t.map(|t| t.top_tube_below_head_tube_top),
        Field::DownTubeJunction => t.map(|t| t.down_tube_above_head_tube_bottom),
        Field::SeatTubeOverhang => t.map(|t| t.seat_tube_overhang),
        Field::SeatTubeBendRadius => t.map(|t| t.seat_tube_bend_radius),
        Field::SeatTubeInsertion => t.map(|t| t.seat_tube_insertion),
        Field::TubeDiameter(k) => t.and_then(|t| t.tube(k)).map(|t| t.outside_diameter),
        Field::TubeWall(k) => t.and_then(|t| t.tube(k)).map(|t| t.wall),
        Field::CrankLength => g.map(|g| g.crank_length),
        Field::PedalLength => g.map(|g| g.pedal_length),
        Field::PedalThickness => g.map(|g| g.pedal_thickness),
        Field::ForkAxleToCrown => g.map(|g| g.fork_axle_to_crown),
        Field::ForkOffset => g.map(|g| g.fork_offset),
        Field::Standover => p
            .tubes
            .standover
            .pinned()
            .or_else(|| crate::scene::standover(&doc.solved)),
    }
}

/// The material choice (H6): `None` is the default.
pub fn set_material(doc: &Document, material: Option<Material>) -> Command {
    let mut tubes = doc.project.tubes.clone();
    tubes.material = material;
    Command::SetTubes(tubes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solved(doc: &mut Document) -> Vec<FieldView> {
        let data = Data::builtin();
        doc.solve(&data);
        fields(doc, &data, LengthUnit::Millimeters)
    }

    fn view(views: &[FieldView], field: Field) -> FieldView {
        views.iter().find(|v| v.field == field).unwrap().clone()
    }

    #[test]
    fn both_seat_angles_and_every_variable_have_a_row() {
        let views = solved(&mut Document::new());
        for v in V::ALL {
            let row = view(&views, Field::Geometry(v));
            assert_eq!(row.locked, Some(false));
            assert!(row.dimmed && !row.text.is_empty(), "{v:?}");
        }
        assert!(views.iter().any(|v| v.label == "Effective seat angle"));
        assert!(views.iter().any(|v| v.label == "Seat tube angle"));
    }

    #[test]
    fn typing_into_a_free_field_pins_it_and_unchanged_text_does_nothing() {
        let mut doc = Document::new();
        let views = solved(&mut doc);
        let reach = view(&views, Field::Geometry(V::Reach));
        let mm = LengthUnit::Millimeters;
        assert_eq!(commit(&doc, &reach, &reach.text, mm).unwrap(), None);
        assert_eq!(
            commit(&doc, &reach, "480", mm).unwrap(),
            Some(Command::PinGeometry(V::Reach, 480.0))
        );
        // Cleared by accident: nothing changes, and the field shows its value again.
        assert_eq!(commit(&doc, &reach, "", mm).unwrap(), None);
        assert_eq!(commit(&doc, &reach, "  ", mm).unwrap(), None);
        assert!(commit(&doc, &reach, "4x", mm).is_err());

        let angle = view(&views, Field::Geometry(V::HeadTubeAngle));
        assert_eq!(
            commit(&doc, &angle, "64", mm).unwrap(),
            Some(Command::PinGeometry(V::HeadTubeAngle, 64.0))
        );
    }

    #[test]
    fn inches_are_converted_and_travel_stays_in_millimetres() {
        let mut doc = Document::new();
        let data = Data::builtin();
        doc.solve(&data);
        let views = fields(&doc, &data, LengthUnit::Inches);
        let stack = view(&views, Field::Geometry(V::Stack));
        assert_eq!(stack.unit, "in");
        let Some(Command::PinGeometry(_, mm)) =
            commit(&doc, &stack, "25", LengthUnit::Inches).unwrap()
        else {
            panic!()
        };
        assert!((mm - 635.0).abs() < 1e-9);
        let travel = view(&views, Field::Travel(End::Front));
        assert_eq!(travel.unit, "mm");
        assert_eq!(
            commit(&doc, &travel, "150", LengthUnit::Inches).unwrap(),
            Some(Command::SetTravel(End::Front, Some(Millimeters(150.0))))
        );
        assert_eq!(
            commit(&doc, &travel, "0", LengthUnit::Inches).unwrap(),
            Some(Command::SetTravel(End::Front, None)),
            "0 is rigid"
        );
    }

    #[test]
    fn the_lock_pins_at_the_shown_value_and_frees() {
        let mut doc = Document::new();
        solved(&mut doc);
        let Some(Command::PinGeometry(V::Stack, value)) =
            toggle_lock(&doc, Field::Geometry(V::Stack))
        else {
            panic!()
        };
        let shown = doc
            .solved
            .geometry
            .as_ref()
            .unwrap()
            .static_state
            .value(V::Stack);
        assert_eq!(value, shown);
        doc.apply(Command::PinGeometry(V::Stack, value));
        assert_eq!(
            toggle_lock(&doc, Field::Geometry(V::Stack)),
            Some(Command::FreeGeometry(V::Stack))
        );
        assert_eq!(toggle_lock(&doc, Field::CrankLength), None);
    }

    #[test]
    fn frame_fields_set_and_clear_overrides() {
        let mut doc = Document::new();
        let views = solved(&mut doc);
        let mm = LengthUnit::Millimeters;
        let seat = view(&views, Field::SeatTubeLength);
        assert!(seat.is_default && seat.locked == Some(false));
        let cmd = commit(&doc, &seat, "440", mm).unwrap().unwrap();
        doc.apply(cmd);
        assert_eq!(doc.project.tubes.seat_tube_length, Parameter::Pinned(440.0));

        let views = solved(&mut doc);
        let wall = view(&views, Field::TubeWall(TubeKind::DownTube));
        doc.apply(commit(&doc, &wall, "1.2", mm).unwrap().unwrap());
        assert_eq!(
            doc.project.tubes.sections[&TubeKind::DownTube].wall,
            Some(Millimeters(1.2))
        );
        let views = solved(&mut doc);
        let wall = view(&views, Field::TubeWall(TubeKind::DownTube));
        assert!(!wall.dimmed);
        // Clearing it keeps the override.
        assert_eq!(commit(&doc, &wall, "", mm).unwrap(), None);

        let views = solved(&mut doc);
        let pedal = view(&views, Field::PedalLength);
        doc.apply(commit(&doc, &pedal, "110", mm).unwrap().unwrap());
        let set = doc.project.crank.pedal.unwrap();
        assert_eq!(set.length, Millimeters(110.0));
        assert!(set.thickness.0 > 0.0);

        doc.apply(set_material(&doc, Some(Material::Titanium)));
        assert_eq!(doc.project.tubes.material, Some(Material::Titanium));
    }

    #[test]
    fn fork_rows_set_overrides_and_standover_pins() {
        let mut doc = Document::new();
        let views = solved(&mut doc);
        let offset = view(&views, Field::ForkOffset);
        assert_eq!(offset.letter, Some('M'));
        let command = commit(&doc, &offset, "51", LengthUnit::Millimeters).unwrap();
        assert!(matches!(
            command,
            Some(Command::SetFork(f)) if f.offset == Some(Millimeters(51.0))
        ));
        let standover = view(&views, Field::Standover);
        assert!(!standover.read_only && standover.locked == Some(false));
        assert!(matches!(
            commit(&doc, &standover, "780", LengthUnit::Millimeters).unwrap(),
            Some(Command::SetTubes(t)) if t.standover == Parameter::Pinned(780.0)
        ));
        for v in [V::FrontCentre, V::RearCentre] {
            let row = view(&views, Field::Geometry(v));
            assert!(!row.read_only && row.locked == Some(false), "{v:?}");
        }
        let overhang = view(&views, Field::SeatTubeOverhang);
        assert_eq!(
            (overhang.letter, overhang.field.group()),
            (Some('S'), Some(1))
        );
        assert!(matches!(
            commit(&doc, &overhang, "60", LengthUnit::Millimeters).unwrap(),
            Some(Command::SetTubes(t)) if t.junctions.seat_tube_overhang == Some(Millimeters(60.0))
        ));
        let letters: Vec<char> = views
            .iter()
            .filter(|v| v.section == Section::Geometry)
            .filter_map(|v| v.letter)
            .collect();
        assert_eq!(letters.iter().collect::<String>(), "ABDNFELSKGHRIQJMO");
        // Each group's rows sit next to each other, so one box holds them.
        let groups: Vec<Option<u8>> = views
            .iter()
            .filter(|v| v.section == Section::Geometry)
            .map(|v| v.field.group())
            .collect();
        let mut runs: Vec<Option<u8>> = groups.clone();
        runs.dedup();
        let boxed: Vec<u8> = runs.into_iter().flatten().collect();
        assert_eq!(boxed, [0, 1, 2, 3, 4]);
        assert_eq!(groups.iter().flatten().count(), 14);
    }

    /// A seat tube angle past the effective is flagged, so the panel can
    /// block it (S-TUBES B6).
    #[test]
    fn seat_tube_angle_past_the_effective_is_not_allowed() {
        let mut doc = Document::new();
        doc.apply(Command::PinGeometry(V::EffectiveSeatTubeAngle, 75.0));
        doc.apply(Command::PinGeometry(V::ActualSeatTubeAngle, 74.0));
        let views = solved(&mut doc);
        let actual = view(&views, Field::Geometry(V::ActualSeatTubeAngle));
        let effective = view(&views, Field::Geometry(V::EffectiveSeatTubeAngle));
        let unit = LengthUnit::Millimeters;
        assert!(not_allowed(&doc, &actual, "75.1", unit).is_some());
        assert!(not_allowed(&doc, &actual, "75", unit).is_none());
        assert!(not_allowed(&doc, &actual, "abc", unit).is_none());
        assert!(not_allowed(&doc, &effective, "73.9", unit).is_none());
        assert!(not_allowed(&doc, &effective, "76", unit).is_none());
    }

    /// Regression: a negative BB drop was read as a nudge, so stepping or
    /// typing "-30" subtracted 30 from the current value.
    #[test]
    fn a_negative_bb_drop_is_a_value_not_a_nudge() {
        let mut doc = Document::new();
        doc.apply(Command::PinGeometry(V::BottomBracketDrop, -29.0));
        let views = solved(&mut doc);
        let bb = view(&views, Field::Geometry(V::BottomBracketDrop));
        let mm = LengthUnit::Millimeters;
        assert_eq!(
            commit(&doc, &bb, "-30", mm).unwrap(),
            Some(Command::PinGeometry(V::BottomBracketDrop, -30.0))
        );
        for up in [true, false] {
            let next = step(&bb, &bb.text, up, false, mm).unwrap();
            let want = if up { -28.0 } else { -30.0 };
            assert_eq!(
                commit(&doc, &bb, &next, mm).unwrap(),
                Some(Command::PinGeometry(V::BottomBracketDrop, want)),
                "{next}"
            );
        }
        // Other lengths still nudge.
        let reach = view(&views, Field::Geometry(V::Reach));
        let current = shown_value(&doc, reach.field).unwrap();
        assert_eq!(
            commit(&doc, &reach, "+10", mm).unwrap(),
            Some(Command::PinGeometry(V::Reach, current + 10.0))
        );
    }

    #[test]
    fn arrows_step_angles_lengths_and_inches() {
        let mut doc = Document::new();
        let views = solved(&mut doc);
        let head = view(&views, Field::Geometry(V::HeadTubeAngle));
        assert_eq!(
            step(&head, "64", true, false, LengthUnit::Millimeters).as_deref(),
            Some("64.1")
        );
        assert_eq!(
            step(&head, "64", false, true, LengthUnit::Millimeters).as_deref(),
            Some("63.0")
        );
        let travel = view(&views, Field::Travel(End::Front));
        // Travel stays in millimetres even when the display is in inches.
        assert_eq!(
            step(&travel, "150", true, false, LengthUnit::Inches).as_deref(),
            Some("151.0")
        );
        let crank = view(&views, Field::CrankLength);
        assert_eq!(
            step(&crank, "6.5", true, false, LengthUnit::Inches).as_deref(),
            Some("6.600")
        );
        assert_eq!(step(&crank, "x", true, false, LengthUnit::Inches), None);
        let standover = view(&views, Field::Standover);
        assert_eq!(
            step(&standover, "800", true, false, LengthUnit::Millimeters).as_deref(),
            Some("801.0")
        );
    }
}

#[cfg(test)]
mod clearing {
    use super::*;
    use crate::document::{Data, Document};

    /// Regression: freeing a field of a bike with low seat angles left no
    /// geometry, because the seat tube angle's default (74°) passed the
    /// effective angle. Clearing a field now changes nothing, and unlocking
    /// any field keeps the model.
    #[test]
    fn clearing_or_unlocking_any_field_keeps_the_model() {
        let data = Data::builtin();
        let mut doc = Document::new();
        doc.solve(&data);
        let g = doc.solved.geometry.clone().unwrap();
        for v in [
            V::Reach,
            V::Stack,
            V::HeadTubeAngle,
            V::ChainstayLength,
            V::BottomBracketDrop,
        ] {
            doc.apply(Command::PinGeometry(v, g.static_state.value(v)));
        }
        doc.apply(Command::PinGeometry(V::EffectiveSeatTubeAngle, 72.0));
        doc.apply(Command::PinGeometry(V::ActualSeatTubeAngle, 71.0));
        doc.solve(&data);
        let views = fields(&doc, &data, LengthUnit::Millimeters);
        for view in views.iter().filter(|v| !v.read_only) {
            let mut d = Document::with_project(doc.project.clone(), None);
            d.solve(&data);
            assert_eq!(commit(&d, view, "", LengthUnit::Millimeters), Ok(None));
            if let Some(command) = toggle_lock(&d, view.field).filter(|_| view.locked == Some(true))
            {
                d.apply(command);
                d.solve(&data);
                assert!(
                    d.solved.geometry.is_some() && d.solved.tubes.is_some(),
                    "unlocking {:?}: {:?}",
                    view.field,
                    d.solved.diagnostics
                );
            }
        }
    }
}
