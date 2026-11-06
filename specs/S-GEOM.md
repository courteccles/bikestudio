# S-GEOM — Geometry Solver and Constraint Diagnostics

**Status:** approved 2026-11-02. **Source:** plan section 1 (`bike-geometry`, `bike-defaults`), section 2 (`GeometryInputs`, `Solved`, design principles 2, 4 and 6), D3, D14, D19, D20 in `../decisions.md`, S-REF R1, R6 and R7.

## Purpose

Turn the geometry the designer pins (head angle, reach, stack and so on) into a complete static frame: every geometry value, the wheel and contact points, and diagnostics for anything over- or under-constrained. This settles D3's frame geometry, and every later spec draws from it.

## How the geometry fits together

With the wheel sizes and the fork known, a static frame on flat ground has **5 degrees of freedom** in the front and rear, plus the 2 seat angles, and these variables:

| Variable | Notes |
|---|---|
| Stack, Reach | Head tube top centre, from the crank axis. |
| Head tube angle | |
| Effective seat angle | Sets the saddle point (G4). Independent of the others. |
| Actual seat angle | Added for G4; with the saddle point, sets the seat tube axis. Independent of the others. |
| Chainstay length | Crank axis to rear axle, straight line. |
| BB drop | Vertical distance from the midpoint of the line between the axles down to the crank centre (G8). |
| BB height | Crank axis above the ground. |
| Wheelbase | Rear axle to front axle, horizontal. |
| Head tube length | Added to the plan's list (G2). |

Three equations tie them together, which is why only 6 are free:

1. **BB height** = the average of the two wheel radii − BB drop (G8).
2. **Front axle height:** the fork, hanging from the bottom of the head tube, must put the front axle at the front wheel's radius above the ground. This links stack, head tube length, head angle, BB drop and the fork.
3. **Wheelbase** = rear centre + front centre, which links reach, stack, head angle, head tube length, chainstay, BB drop and the fork.

The saddle point, the seat tube axis, trail, front centre, rear centre and fork length are derived outputs, never pinned in version 1.

## Decisions

| # | Decision | Status |
|---|---|---|
| G1 | **Scope of "at sag":** S-GEOM solves static geometry and, for hardtails, geometry at fork sag (the front drops along the steering axis and the bike pivots about the rear contact patch). A rigid bike has no sag, so its static geometry is its riding geometry. Full-suspension geometry at sag and at full travel, including pedal clearance (G13), needs the rear wheel position from the linkage, so S-KINEMATICS adds it. | Confirmed |
| G2 | Add **head tube length** to `GeometryVariable`. Every published chart lists it, and without it the front of the bike isn't fixed. | Confirmed |
| G3 | **Fork inputs** are axle-to-crown and offset. Head tube length includes the lower headset stack, so there's no separate input for it; a chart fixture that lists the lower headset stack separately adds it to the head tube length and records that. Until S-COMPONENTS adds a fork library, the fork inputs come from `bike-defaults/fork.toml` by front travel and wheel size, and the project can override each one. A rigid fork is 0 mm of travel in the same table. | Confirmed |
| G4 | **Seat angles.** The **saddle point** is where the effective seat line, drawn up from the crank centre, crosses the horizontal line through the top of the head tube (stack height). The **effective seat angle** is the acute angle between the effective seat line and that horizontal. The **actual seat angle** is measured against the same horizontal, and the seat tube axis is the line through the saddle point at that angle. Both are pinnable variables. Seat tube length comes with S-TUBES. | Confirmed |
| G5 | **Choosing what to solve:** for each equation, the solver takes the first free variable in a fixed order: (1) BB height, BB drop; (2) head tube length, stack, head tube angle, BB drop; (3) wheelbase, reach, stack, head tube angle, chainstay. A free variable that no equation needs is filled from `bike-defaults/geometry.toml` and labelled as a default, as the mockups show ("preview from defaults"). | Confirmed |
| G6 | **Over-constrained:** when an equation has no free variable, its pinned values are checked against each other. A mismatch over 1 mm or 1° is an `Error` diagnostic naming the pinned values involved and how far apart they are, for example "Wheelbase 1260.0 disagrees with reach, stack and head angle by 4.2 mm. Free one of them." The rest still solves. | Confirmed |
| G7 | **Never blocks** (design principle 6): with no wheel size, no pins or impossible values (a chainstay shorter than the BB drop, for example), the solver returns everything it can and diagnostics for the rest. It never panics. | Confirmed |
| G8 | **BB drop** is measured vertically from the midpoint of the line between the two axle centres down to the BB centre (the crank centre). So BB height = the average of the two wheel radii − BB drop, which works the same for equal wheels and mullets. | Confirmed |
| G9 | **Crates:** S-GEOM creates `bike-geometry` (the solver) and `bike-defaults` (`wheel_size.toml`, `fork.toml`, `geometry.toml`). `Diagnostic`, `Severity`, `Solved` and `SolvedGeometry` go in `bike-model`, as the plan lays out. | Confirmed |
| G10 | **The published charts come first, from Nicolai:** S-REF's chart fixtures (deliverable 4) don't exist yet, and S-GEOM is their first user, so S-GEOM gathers them. The charts are Nicolai's official tech sheets, which give axle-to-crown, fork offset, tyre diameter and lower headset stack: the Nicolai Argon GX 2025 (rigid gravel bike), the Nicolai Argon GTB GPI 2025 (hardtail with a Pinion gearbox), the Nicolai G1 with 29" wheels and as a 29"/27.5" mullet (full suspension) and the Nicolai S16 MGU 2026 (full-suspension e-bike with a Pinion motor and gearbox). Nicolai publishes static geometry only, so the charts test static geometry; geometry at fork sag is tested against hand calculations. The reference calculator and linkage fixtures stay with S-KINEMATICS, their first user. | Confirmed |
| G11 | **Defaults are sensible starting values:** `geometry.toml` and `fork.toml` give a realistic bike for each wheel size and travel band, so the default solve (G12) gives the designer a good idea of what each value could be. Every entry cites its source. | Confirmed |
| G12 | **A new project starts from a default solve:** instead of an empty drawing that fills in as values are entered, a new project shows a complete bike from `geometry.toml`, for example 29" wheels and a 65° head angle, with every value labelled as a default. The designer starts from a believable bike and pins values over the defaults. The defaults are shown, not saved: the project file stays empty until the designer sets something (D20). | Confirmed |
| G13 | **Crank and pedal clearance:** crank length and a pedal (default platform 100 mm long and 20 mm thick, centred on the pedal axle) are inputs. The default crank length follows rider size, from a table in `geometry.toml` keyed by the rider size preset (for example Small 165 mm, Medium 170 mm, Large 175 mm); until S-MASS adds the rider, it uses Medium. The crank and pedal are drawn in two positions: **straight down**, giving pedal clearance to the ground (BB height − crank length − half the pedal thickness), and **forward horizontal**, giving the gap from the pedal's front edge to the front tyre with the wheel straight ahead. Clearances are given static, at sag and at full travel: for hardtails, at fork sag and full fork travel (S-GEOM); for full suspension, at sag and full travel (S-KINEMATICS, which has the rear wheel position). A clearance of 0 or less is a `Warning` diagnostic. The pedal only, not the foot. | Confirmed |

## Scope

1. **`bike-model` additions:** `GeometryInputs` (an `EnumMap`-style map from `GeometryVariable` to `Parameter`), `Parameter { Pinned(f64), Free }`, fork overrides (axle-to-crown and offset, each optional), crank length and pedal size (optional, G13), `Diagnostic`, `Severity`, `Solved`, `SolvedGeometry`. `Project` gains `geometry`. `schema_version` stays 1 (M4: no compatibility promise yet), and the save/load tests cover the new fields.
2. **`bike-defaults`:** the three TOML files, each entry with a source, loaded and validated at startup. A malformed file is a test failure, not a runtime crash.
3. **`bike-geometry`:** `solve_geometry(&Project, &Defaults) -> (Option<SolvedGeometry>, Vec<Diagnostic>)`.
   - `SolvedGeometry` holds every variable static and at fork sag, each marked `Pinned`, `Solved` or `Default`, plus trail, front centre, rear centre, fork length, and the points: crank axis, both axles, both contact patches, head tube top and bottom, the saddle point, and the crank and pedal in both G13 positions, plus pedal-to-ground and pedal-to-front-tyre clearance.
   - Each equation is solved in closed form where its unknown is a length, and with a bracketed 1D root finder where it's an angle.
4. **Diagnostic codes:** `geometry.unpinned` (Info, as in the mockups), `geometry.default` (Info, per defaulted value), `geometry.conflict` (Error, G6), `geometry.impossible` (Error, G7), `wheel.missing` (Info), `fork.default` (Info, as in the mockups), `pedal.ground` and `pedal.front_tyre` (Warning, G13).
5. **Chart fixtures:** the five Nicolai charts (G10) in `reference-data/geometry/`, one TOML each with every size, the source web address, date retrieved, fork used and any R7 corrections. Where a chart gives a range or a single-size value (the Nicolai Argon GTB GPI's adjustable chainstay; its head and seat angles listed once), the fixture records which value it uses and why.
6. **Out of scope:** full-suspension geometry at sag (S-KINEMATICS), tubes and seat tube length (S-TUBES), the fork library (S-COMPONENTS), the specification pane and side view (S-SHELL).

## Done when

- For every chart fixture and size, pinning 5 independent chart values, plus the seat angles the chart gives, with the chart's fork reproduces every other chart value within ±3 mm and ±0.3° (R6), apart from values recorded as known gaps in the chart file (S-REF "Changes after approval"), whose gaps must not move by more than 0.2 mm. Charts that omit a value (the Nicolai G1 has no BB drop) pin wheelbase instead, and the solved BB drop and front centre are checked against the chart where it gives them.
- Geometry at fork sag matches hand calculations, written out in the test, for a hardtail. A rigid frame has no fork sag, and full suspension has no sag state until S-KINEMATICS.
- **Round trip:** for every fixture, solving with any valid choice of independent pins and then pinning the solved values in a different valid set reproduces the same frame within 0.01 mm and 0.001°.
- An empty project, a project with only wheel sizes, and every conflicting or impossible combination in the tests return diagnostics, never a panic. A property test feeds random pins and checks the same.
- Pedal clearances match hand calculations for a hardtail, static, at fork sag and at full fork travel, and a pedal touching the ground or the tyre raises its warning.
- Save/load round-trips a project with pinned geometry and fork overrides.
- CI passes on Linux and macOS.

## Changes after approval

- 2026-11-02: Corrected errors left from drafting: BB drop is defined as in G8, the frame has 5 degrees of freedom, actual seat tube angle is in scope (G4), the sag hand calculation covers hardtails only, and chart values recorded as known gaps are exempt from the ±3 mm check.
- 2026-11-04 (S-SHELL amendment): defaults are no longer reported as diagnostics; the panel labels them from `ValueSource`. `wheel.missing`, `fork.default`, `geometry.unpinned` and `geometry.default` are removed.
- 2026-11-05 (S-TUBES B6): the actual seat angle can't be steeper than the effective; after solving, that's a `geometry.impossible` error.
- 2026-11-06 (S-TUBES B6): a free actual seat angle's default is capped at the effective seat angle, so clearing it can't break the solve.
- 2026-11-05 (S-EDIT-CENTERS E1): front centre and rear centre are now pinnable geometry variables (`FrontCentre`, `RearCentre`, labelled Front center and Rear center), replacing "never pinned in version 1". Equation 3 splits into three: (3a) chainstay² = rear centre² + rear axle height², (3b) front centre = the front axle's horizontal distance ahead of the crank centre, and (3c) wheelbase = rear centre + front centre. With two more variables and two more equations, the number of free values stays the same. G5 order: (3a) rear centre, chainstay; (3b) front centre, reach, stack, head angle; (3c) wheelbase, front centre, rear centre, then reach, stack, head angle, chainstay. A chart test pins front and rear centre instead of reach and chainstay.

## Open questions

- None.
