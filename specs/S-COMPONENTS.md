# S-COMPONENTS — Component Data and Templates

**Status:** approved 2026-11-03, with C10 amended after the data search. **Source:** plan section 1 (`bike-components`, `bike-defaults`), section 2 (`Components`, `ComponentReference`, `DriveSystem`, `DriveUnitComponent`), design principles 5 and 7, D10, D15, D19 in `../decisions.md`, S-GEOM G3, S-TUBES T6.

## Purpose

Give bikestudio a library of real components (forks, shocks, drive units and batteries) that a design can select instead of typing dimensions in. A selected component sets exact dimensions, an outline and a mass. Nothing requires one (design principle 7): with nothing selected, the defaults from `bike-defaults` stand in, as they do now.

This spec covers the data, how a project refers to it, and how a selection turns into values for the solvers. Drawing components is S-SHELL; using shock data is S-LINK and S-KINEMATICS; using masses is S-MASS.

## Decisions

| # | Decision | Status |
|---|---|---|
| C1 | **Component kinds in version 1:** forks, shocks, drive units (gearboxes, mid-drive motors and integrated motor-gearboxes such as the Pinion MGU) and batteries. Every entry has a **make, model and year**, a source, a **mass and its centre of mass location** (in the component's own coordinates, so it moves with the component), and a **visual representation** that can be drawn on the side view (C13). Tyres are out (D19). Cockpit components (stem, bar and saddle) move to S-MASS, the first spec that needs the hands and saddle. Saddle setback is a fit setting in S-MASS, not a component. Until then crank and pedal stay where S-GEOM put them. | Confirmed |
| C2 | **Crate and files:** a new `bike-components` crate holds the data and its loader, depending only on `bike-model` (plan section 1). Data is TOML, one file per manufacturer per kind, for example `data/forks/rockshox.toml` and `data/drive_units/pinion.toml`, so a community pull request (D15) touches one small file. Files are compiled in with `include_str!` like `bike-defaults`, and every file is parsed and validated by the tests. | Confirmed |
| C3 | **Referring to a component:** `ComponentReference<T>` is `Library(ComponentId)` or `Custom(T)` (plan). A `ComponentId` is a stable string `make/model/year/variant`, for example `rockshox/zeb/2025/29-170-44`. A custom component has the same fields as a library one, so a builder with an OEM drawing can enter it. A project that refers to an id the library no longer has still loads; the solve gets a `component.unknown` warning and falls back to the defaults. | Confirmed |
| C4 | **Forks:** each library entry is one make, model and year with its variants (wheel size × travel × offset). Each variant has travel, axle-to-crown, offset, steerer (tapered or straight, with its diameters), mass, centre of mass and source. The fork has a single **offset**, the perpendicular distance from the steering axis to the axle, as makers publish it; it's used for the geometry and the drawing. A selected fork replaces `fork.toml`; the `Project.fork` overrides stay as the way to adjust it (for example an air shaft change). If the fork's travel differs from `front_travel`, the fork's travel is used and a `fork.travel_mismatch` warning says so. | Confirmed |
| C5 | **Shocks:** make, model, year, eye-to-eye, stroke, mount (standard eyelet or trunnion), spring type (air or coil), mass, centre of mass and source. S-COMPONENTS stores and validates them; S-LINK uses eye-to-eye and stroke. Each eye-to-eye × stroke size a model comes in is a variant. Named shocks only, no generic size-standard entries; a design with no shock selected uses the stroke the linkage needs (plan), as now. | Confirmed |
| C6 | **Drive units** (plan `DriveUnitComponent`, design principle 5): make, model, year; power output (rated power, peak power and torque for motors; none for a gearbox); the **bolt pattern** (mounting bolt positions and diameters); the dimensions that **locate the crank centre output** relative to the bolt pattern; the outline; optional output sprocket (teeth) and ratios; mass with centre of mass. All in the unit's own coordinates with the crank centre at the origin. A design places it with a rotation about the crank axis. One physical unit is one entry, so the Pinion MGU is one body. | Confirmed |
| C7 | **Where drive unit data comes from.** Pinion publishes frame interface drawings; most motor makers give outlines only to frame makers under agreement. Library entries are added only where a public source gives the outline and mounts, and each says which document and revision. An outline the source gives only roughly (for example from a published side-view drawing with a few dimensions) is marked `outline_accuracy = "approximate"` and the solve shows a `drive_unit.approximate` info diagnostic. A builder with an OEM drawing enters a custom unit (C3). No outline is ever estimated from photos. | Confirmed |
| C8 | **A drive unit in the frame** (S-TUBES T6): when a unit is selected, the bottom bracket shell is removed, and the down tube and chainstays end where their centrelines meet the unit's outline instead of at the crank centre. The unit's mass is a component mass for S-MASS, not frame mass; S-TUBES's fittings stay as they are. A gearbox or motor frame also gets a mounting plate mass (the frame-side bracket the unit bolts to): a per-unit `frame_mount_mass` in the drive unit entry, counted as frame mass, 0 when the source doesn't say. | Confirmed |
| C9 | **Batteries:** make, model, year; power information (capacity in Wh, nominal voltage, and Ah where given); dimensions (length, width, height); mass and centre of mass; outline; source. A design places it along the down tube: distance from the down tube's lower end to the battery's near end, and inside or under the tube. Its centre of mass is the entry's, moved with the battery. With no battery selected and a motor fitted, nothing is assumed and an info diagnostic says so. Free placement anywhere in the frame is later. | Confirmed |
| C10 | **First library contents** (amended 2026-11-03): the Nicolai sheets name no fork or shock models and Pinion publishes no frame interface drawing for the C1.12 or the MGU E1.12, so the first library holds current models that fit the Nicolai sheets' sizes, from the makers' published data: a RockShox SID SL (32 mm stanchions, the Argon GTB GPI's limit), a RockShox Lyrik (29", 160 mm, 44 mm offset, for the S16 MGU) and a RockShox Super Deluxe 205 × 62.5 trunnion (the S16 MGU's shock). No drive units or batteries ship until a public source exists (C7); the custom path (C3) covers them. Other makers come from pull requests (D15). Each entry is checked against its source in review. A centre of mass the source doesn't give is the centroid of the component's drawing (C13), and the entry says so. | Confirmed |
| C11 | **"Renders" in the plan's test** means every entry produces valid side-view geometry: a closed, non-self-intersecting outline, mounts inside or on it, and positive sizes. S-SHELL draws it. There's no shell to render with yet. | Confirmed |
| C12 | **Resolution** (plan section 1): a `resolve` step turns each `ComponentReference` into concrete values, falling back to `bike-defaults` where nothing is selected, before `solve_geometry` and `solve_tubes` run. Until S-SHELL adds `bike-application`, `resolve` lives in `bike-components` and the solvers take the resolved values. Rear drive (cassette or single cog) and idlers wait for S-FORCES, their first user. | Confirmed |
| C13 | **Visual representation:** each component's side view is drawn from data, in the component's own coordinates. **Forks and shocks** are drawn parametrically from a few published dimensions (fork: stanchion diameter, lower leg diameter, crown height, arch, offset; shock: body and can diameters, eye or trunnion size, reservoir if any), so every entry looks like its real part without a hand-made drawing, and the drawing follows travel and sag. **Drive units and batteries** carry an outline polygon taken from their source drawing (C7). Any entry may add an `svg_path` override, an SVG path in the same coordinates, for a more detailed drawing. | Confirmed |

## Scope

1. **`bike-model` additions:** `ComponentId`, `ComponentReference<T>`, `Fork`, `Shock`, `DriveUnitComponent`, `Battery`, `DriveUnit` (reference + rotation), `BatterySetup` (reference + placement), `Components` (fork, shock), and `Project` gains `components` and `drive` (drive unit, battery). `schema_version` stays 1; save/load tests cover the new fields.
2. **`bike-components`:** the TOML files (C2, C10), loader, validation and `resolve` (C12).
3. **`bike-geometry`:** `solve_geometry` uses a resolved fork (C4); `solve_tubes` handles a drive unit outline (C8).
4. **Diagnostic codes:** `component.unknown` (Warning, C3), `component.invalid` (Error: a custom component fails validation, C3), `fork.travel_mismatch` (Warning, C4), `drive_unit.approximate` (Info, C7), `battery.none` (Info, C9), `drive_unit.tube_misses` (Error: a tube centreline doesn't meet the outline, C8).
5. **Out of scope:** drawing components (S-SHELL), shock use (S-LINK, S-KINEMATICS), masses in the centre of mass (S-MASS), rear drive and idlers (S-FORCES), cockpit components and saddle (S-MASS, C1), tyres (D19), hub motors, CVTs and hub gears (D10).

## Done when

- Every library entry parses, cites a source and produces valid side-view geometry (C11).
- A fork selected from the library gives the same solved geometry as typing its axle-to-crown and offset as overrides, and a travel mismatch warns (C4).
- The Nicolai Argon GTB GPI and the Nicolai S16 MGU solve with a test-only custom drive unit (synthetic, labelled as such, not in `data/`): no bottom bracket shell, the down tube and chainstays end on the outline, and their chart geometry still reproduces as in S-GEOM (C10).
- The Nicolai charts solve with the library fork selected and the sheet's axle-to-crown kept as an override, so S-GEOM results don't move.
- A project referring to an unknown component loads, warns and solves from defaults.
- Save/load round-trips a project with library and custom components.
- CI passes on Linux and macOS.

## Changes after approval

- 2026-11-05 (C13): a rigid fork's crown is centred on the steering axis, not offset. Each blade leaves the crown along the steering axis, then curves forward in one direction (no S bend) to the axle, and ends in a rounded dropout centred on the axle, concentric with the hub. On every fork, axle-to-crown is measured to the top of the crown.

## Open questions

- None.
