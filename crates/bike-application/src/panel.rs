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
    TubeDiameter(TubeKind),
    TubeWall(TubeKind),
    CrankLength,
    PedalLength,
    PedalThickness,
}

/// The panel's sections, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Travel,
    Geometry,
    Frame,
    Tubes,
    Cranks,
}

impl Section {
    pub fn label(self) -> &'static str {
        match self {
            Section::Travel => "Travel",
            Section::Geometry => "Geometry",
            Section::Frame => "Frame",
            Section::Tubes => "Tubes",
            Section::Cranks => "Cranks",
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
        });
    }

    let seat_pinned = p.tubes.seat_tube_length.pinned();
    let seat = seat_pinned.or_else(|| tubes.map(|t| t.seat_tube_length));
    out.push(FieldView {
        field: Field::SeatTubeLength,
        section: Section::Frame,
        label: "Seat tube length".into(),
        text: length(seat, display),
        unit: display.label(),
        locked: Some(seat_pinned.is_some()),
        dimmed: seat_pinned.is_none(),
        is_default: seat_pinned.is_none(),
    });
    let j = &p.tubes.junctions;
    for (field, label, set, solved) in [
        (
            Field::TopTubeJunction,
            "Top tube junction",
            j.top_tube_below_head_tube_top,
            tubes.map(|t| t.top_tube_below_head_tube_top),
        ),
        (
            Field::DownTubeJunction,
            "Down tube junction",
            j.down_tube_above_head_tube_bottom,
            tubes.map(|t| t.down_tube_above_head_tube_bottom),
        ),
        (
            Field::SeatTubeOverhang,
            "Seat tube overhang",
            j.seat_tube_overhang,
            tubes.map(|t| t.seat_tube_overhang),
        ),
    ] {
        out.push(optional_length(
            field,
            Section::Frame,
            label,
            set,
            solved,
            display,
        ));
    }

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

    let pedal = p.crank.pedal;
    out.push(optional_length(
        Field::CrankLength,
        Section::Cranks,
        "Crank length",
        p.crank.crank_length,
        geometry.map(|g| g.crank_length),
        display,
    ));
    out.push(optional_length(
        Field::PedalLength,
        Section::Cranks,
        "Pedal length",
        pedal.map(|p| p.length),
        geometry.map(|g| g.pedal_length),
        display,
    ));
    out.push(optional_length(
        Field::PedalThickness,
        Section::Cranks,
        "Pedal thickness",
        pedal.map(|p| p.thickness),
        geometry.map(|g| g.pedal_thickness),
        display,
    ));
    let _ = data;
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
/// (H4); clearing a field frees it or returns it to the default.
pub fn commit(
    doc: &Document,
    view: &FieldView,
    text: &str,
    display: LengthUnit,
) -> Result<Option<Command>, ParseError> {
    if text.trim() == view.text.trim() {
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
    let current = shown_value(doc, field).map(Millimeters);
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
        Field::TubeDiameter(k) => t.and_then(|t| t.tube(k)).map(|t| t.outside_diameter),
        Field::TubeWall(k) => t.and_then(|t| t.tube(k)).map(|t| t.wall),
        Field::CrankLength => g.map(|g| g.crank_length),
        Field::PedalLength => g.map(|g| g.pedal_length),
        Field::PedalThickness => g.map(|g| g.pedal_thickness),
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
        assert!(views.iter().any(|v| v.label == "Actual seat angle"));
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
        assert_eq!(
            commit(&doc, &reach, "", mm).unwrap(),
            Some(Command::FreeGeometry(V::Reach))
        );
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
        doc.apply(commit(&doc, &wall, "", mm).unwrap().unwrap());
        assert!(doc.project.tubes.sections.is_empty());

        let views = solved(&mut doc);
        let pedal = view(&views, Field::PedalLength);
        doc.apply(commit(&doc, &pedal, "110", mm).unwrap().unwrap());
        let set = doc.project.crank.pedal.unwrap();
        assert_eq!(set.length, Millimeters(110.0));
        assert!(set.thickness.0 > 0.0);

        doc.apply(set_material(&doc, Some(Material::Titanium)));
        assert_eq!(doc.project.tubes.material, Some(Material::Titanium));
    }
}
