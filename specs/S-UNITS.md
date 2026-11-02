# S-UNITS — Unit Display, Input and Export Setting

**Status:** approved 2026-11-01. **Source:** plan section 1 (Units), section 5 (Inspector), D18 in `../decisions.md`.

## Purpose

Let the user see and type lengths in millimetres or inches, and choose the unit exports use, while the model stays in millimetres and degrees. Conversion must never change a stored value on its own.

## Decisions

| # | Decision | Status |
|---|---|---|
| U1 | Storage never changes: `Project`, `Solved` and `.bikestud` files are always millimetres and degrees (D18). Units are a presentation concern only. | Confirmed |
| U2 | Only lengths switch units. Angles are always degrees. Wheel sizes stay nominal labels (20, 26, 27.5, 29, 32) in both units. Mass units are left to S-MASS. | Confirmed |
| U3 | The display unit is an app preference, not project data: it isn't saved in `.bikestud`, doesn't affect `Solved` and doesn't create undo steps (like the display preferences in plan section 5). | Confirmed |
| U4 | The export unit is chosen per export, defaulting to the display unit. S-EXPORT uses it; S-UNITS only provides the type and the conversions. | Confirmed |
| U5 | The conversion factor is exactly 25.4 mm per inch. | Confirmed |
| U6 | Display precision: millimetres 1 decimal (matches the mockups, `630.0`), inches 3 decimals, degrees 1 decimal. Trailing zeros are kept so columns line up. | Confirmed |
| U7 | **No drift rule:** committing a field whose text is unchanged from what was displayed leaves the stored value untouched. Only an actual edit converts the typed value to millimetres and stores it. Switching units back and forth never rewrites the model. | Confirmed |
| U8 | Input accepts a bare number in the display unit, or a number with a unit suffix that overrides it: `mm`, `in` or `"`. So `25in` works while displaying millimetres. | Confirmed |
| U9 | Input accepts simple expressions: `+`, `-`, `*`, `/` and parentheses, plus a leading `+n` or `-n` to nudge the current value (plan section 5: `412.5`, `+2`, `400/2`). The nudge is in the display unit. One parser serves every length field. | Confirmed |
| U10 | Code lives in `bike-model::units`: conversion and formatting are trivial helpers, which the crate allows. No new crate. | Confirmed |
| U11 | No fractional inches in version 1: input like `1 1/8"` is a parse error; type `1.125"`. | Confirmed |
| U12 | Suspension travel (front and rear) is always shown in millimetres, whatever the display unit, as the industry quotes it that way. Every other length follows the display unit, including pivot X/Y in the linkage inspector. A unit suffix still works in travel fields, so `6.7in` is accepted and shown as `170.2`. Travel stays in millimetres in exports too, whatever the export unit. | Confirmed |
| U13 | The unit switch sits in the bottom-right corner of the status bar, across from the save status (`Saved` / `Unsaved changes`) on the left: a two-way `mm` / `in` toggle, one click to change. It isn't in the Settings dialog. | Confirmed |

## Scope

1. **Types:**
   ```rust
   pub enum LengthUnit { Millimeters, Inches }   // Default: Millimeters

   impl Millimeters {
       pub fn to_unit(self, unit: LengthUnit) -> f64;
       pub fn from_unit(value: f64, unit: LengthUnit) -> Millimeters;
   }
   ```
2. **Formatting:** `format_length(Millimeters, LengthUnit) -> String` and `format_angle(Degrees) -> String`, using the U6 precision, plus the unit label (`mm`, `in`, `°`) for field suffixes.
3. **Parsing:** `parse_length(text, display_unit, current: Option<Millimeters>) -> Result<Millimeters, ParseError>` and `parse_angle(text, current: Option<Degrees>) -> Result<Degrees, ParseError>`, with the U8 suffixes and U9 expressions. Errors say what was wrong (`"Unknown unit 'cm'"`, `"Can't divide by zero"`), never panic.
4. **No-drift helper:** `commit_length(text, displayed_text, current, unit)` returns the current value unchanged when `text == displayed_text`, and parses otherwise (U7). S-SHELL calls this for every length field.
5. **Out of scope:** drawing the status-bar toggle (U13) and persisting the preference (S-SHELL), the export dialog (S-EXPORT), mass units (S-MASS).

## Done when

- Every length in the S-REF fixtures survives display → commit unchanged in both units, bit for bit, and survives 1000 mm → in → mm switches without changing.
- A value typed in inches and read back in inches shows exactly what was typed (to 3 decimals).
- `25.4mm`, `1in`, `1"` and `+0.5` (on 24.9 mm, in mm) all parse to 25.4 mm. Malformed input returns an error, never a panic, including under a fuzz/property test.
- CI passes on Linux and macOS.

## Open questions

- None.
