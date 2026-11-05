# S-TUBES — Naive Tubes, Seat Tube Length and Mass Estimate

**Status:** approved 2026-11-02. **Source:** plan section 1 (`bike-geometry`), plan section on mass ("frame mass is estimated from tube volume and material density"), D4, D6, D8, D24 in `../decisions.md`, S-GEOM G4 and scope 6.

## Purpose

Turn S-GEOM's solved points into a frame made of tubes: a centreline for each tube, a generic round section (outside diameter and wall), a length, a mass and a centre of mass. This is D6's naive tube model. It is enough for the side view (S-SHELL), the DXF tube centrelines and the named-values table (S-EXPORT), and frame mass for S-MASS. It isn't fabrication data: no miters, butting, ovalising or tapering.

S-TUBES also adds **seat tube length**, which S-GEOM G4 left for this spec.

## How the tubes fit together

All points are in S-GEOM's static frame, in millimetres, with the crank centre at the origin.

| Tube | From | To |
|---|---|---|
| Head tube | Head tube bottom | Head tube top |
| Top tube | Head tube axis, a set distance below the head tube top (T3) | Seat tube axis, a set distance below the seat tube top (T3) |
| Down tube | Head tube axis, a set distance above the head tube bottom (T3) | Crank centre |
| Seat tube | Where the seat tube axis crosses crank height | Seat tube top, at seat tube length along the axis (T2) |
| Chainstays (pair) | Crank centre | Rear axle |
| Seatstays (pair) | Rear axle | Seat tube axis, at the top tube junction (T3) |
| Bottom bracket shell | Crank centre, across the frame | Fixed width, drawn as a circle in side view |

## Decisions

| # | Decision | Status |
|---|---|---|
| T1 | **Which tubes, by frame type.** Rigid and hardtail frames get all the tubes in the table. A full-suspension frame gets the front triangle only (head tube, top tube, down tube, seat tube, bottom bracket shell) in S-TUBES; its rear members are links, so S-LINK adds their tubes using this spec's section and mass model. | Confirmed |
| T2 | **Seat tube length** is measured along the actual seat tube axis, from where that axis crosses crank height up to the top of the seat tube (centre-to-top, the usual chart convention). It's a pinnable input; when free, its default comes from a table in `geometry.toml` keyed by rider size preset (Medium until S-MASS). It must be at least the top tube junction plus a minimum (T3), or it's a `tubes.impossible` error. Minimum seatpost insertion is not reported; that is seatpost data, for later. | Confirmed |
| T3 | **Junction offsets** place the tube ends that aren't fixed points: top tube below the head tube top (default 25 mm along the head tube axis), down tube above the head tube bottom (default 35 mm), top tube and seatstays below the seat tube top (default 60 mm along the seat tube axis; Nicolai calls this the seat tube overhang). Each is an optional project input with a default in a new `bike-defaults/tubes.toml`. The designer only changes them for unusual frames (sloping top tubes, low seatstays). | Confirmed |
| T4 | **Section and material.** Every tube is round, with an outside diameter and a wall thickness. The project picks one frame material; `tubes.toml` gives each material a density and a default diameter and wall for every tube, and the project can override each tube's diameter and wall. Version 1 materials: steel (chromoly 4130), aluminium (6061), titanium (3Al-2.5V) and a generic carbon entry (density only; the wall is an "equivalent" wall). Every entry cites its source. | Confirmed |
| T5 | **Mass and centre of mass.** Each tube's mass is its centreline length × the area of its section × density, and its centre of mass is the centreline midpoint. Pairs (chainstays, seatstays) count twice, using the side-view length; that understates their length a little because stays spread outwards, which is accepted for a naive model. Frame mass = the sum of the tubes + a fittings allowance per material in `tubes.toml` (dropouts, bosses, cable guides, welds or bonding), placed at a fixed point given per fitting (for example dropouts at the rear axle). | Confirmed |
| T6 | **Gearboxes and drive units.** The down tube and chainstays end at the crank centre in S-TUBES. When S-COMPONENTS adds Pinion gearboxes and mid-drive motors, their mounting outline replaces the bottom bracket shell and those tube ends move to it. So the Nicolai Argon GTB GPI and the Nicolai S16 MGU have a bottom bracket shell until S-COMPONENTS. | Confirmed |
| T7 | **Checks against charts.** Nothing derives seat tube length (T2 makes it an input), so the charts test placement instead: each Nicolai fixture records seat tube length, seat tube overhang (the seat tube above the top tube junction) and frame weight where given. For every chart and size, pinning the seat tube length and overhang puts the seat tube top and the top tube junction on the seat tube axis at those distances within 0.01 mm. Mass is checked against hand calculations only; a published frame weight (the Nicolai Argon GX's 1.90 kg) is reported against the estimate for the frame's material, not tested. | Confirmed |
| T8 | **Where it lives.** `solve_tubes(&Project, Option<&SolvedGeometry>, &Defaults) -> (Option<SolvedTubes>, Vec<Diagnostic>)` in `bike-geometry` (D24). `SolvedTubes` goes in `bike-model` next to `SolvedGeometry`, and `Solved` gains it. Tubes come from the static frame only: they're rigid, so the frame at sag is the same tubes moved. | Confirmed |

## Scope

1. **`bike-model` additions:** `TubeInputs` (frame material, optional seat tube length as a `Parameter`, optional junction offsets, optional per-tube diameter and wall), `Material`, `TubeKind`, `Tube` (kind, start, end, length, diameter, wall, count, mass, centre of mass, each value marked `Pinned` or `Default`), `SolvedTubes` (tubes, fittings, frame mass, frame centre of mass). `Project` gains `tubes`. `schema_version` stays 1, and the save/load tests cover the new fields.
2. **`bike-defaults`:** new `tubes.toml` (materials, densities, default sections, junction offsets, fittings allowances, bottom bracket shell width), and a seat tube length table in `geometry.toml`. Each entry cites a source; loaded and validated like the other defaults.
3. **`bike-geometry`:** `solve_tubes` as in T8.
4. **Diagnostic codes:** `tubes.default` (Info, per defaulted value, as with `geometry.default`), `tubes.impossible` (Error: a wall at or over half the diameter, a zero or negative length, a seat tube shorter than its junctions), `tubes.no_geometry` (Info: S-GEOM gave no points, so there are no tubes).
5. **Chart fixtures:** seat tube length, seat tube overhang and published frame weight added to the Nicolai fixtures where given (T7).
6. **Out of scope:** full-suspension rear members (S-LINK), gearbox and motor mounts (S-COMPONENTS), drawing the tubes (S-SHELL), exporting them (S-EXPORT), tyre clearance (S-CLEARANCE), and anything for fabrication (D6).

## Done when

- Every frame type yields tube centrelines and a mass estimate (plan): a rigid frame and a hardtail get every tube in the table, and a full-suspension frame gets the front triangle.
- For every chart fixture and size, the pinned seat tube length and overhang place the seat tube top and top tube junction on the seat tube axis within 0.01 mm, and the Nicolai Argon GX's frame weight is reported against the aluminium estimate.
- Tube lengths, masses, frame mass and centre of mass match hand calculations, written out in the test, for one rigid frame in each material.
- Every tube end lies on its axis (head tube or seat tube) or at its fixed point within 0.001 mm.
- An empty project, a project with no geometry, and every impossible section or length in the tests return diagnostics, never a panic; the S-GEOM property test is extended to call `solve_tubes`.
- Save/load round-trips a project with a material, a pinned seat tube length and tube overrides.
- CI passes on Linux and macOS.

## Changes after approval

- 2026-11-02: T7 changed. Seat tube length is an input, so a chart "reproduction" would only echo the pin; the charts now test seat tube top and junction placement using Nicolai's seat tube length and overhang. The T3 seat tube junction default rose from 40 to 60 mm to match Nicolai's overhangs.
- 2026-11-02 (build): `solve_tubes` takes `Option<&SolvedGeometry>`, so it can return `tubes.no_geometry` itself when S-GEOM gave nothing.
- 2026-11-04 (S-SHELL amendment): defaults are no longer reported as diagnostics; the panel labels them from `ValueSource`. `tubes.default` is removed.

## Open questions

- None.
