# bikestudio — Confirmed Decisions

Decisions confirmed by the project owner during the goal interview. Each one is explicitly verified. The architecture plan is in `bikestudio-plan.md`.

## Goal

bikestudio is a free, open-source Linux and macOS app where boutique and hobby builders settle rigid, hardtail, or full suspension frame geometry and pivots. It exports a drawing and CAD-ready data for detailed design.

## Product

| # | Decision | Status |
|---|---|---|
| D1 | bikestudio is a tool for other frame designers, not a personal or learning project. | Confirmed |
| D2 | Primary user: small custom and boutique builders. Secondary: hobbyist home builders. The primary user wins conflicts. | Confirmed |
| D3 | Priority 1, decide: settle frame geometry and, for full-suspension frames, the linkage pivot positions. | Confirmed |
| D4 | Priority 2, hand off: export a dimensioned 2D side-view drawing and a CAD-consumable data file containing the settled geometry, pivot points and tube centerlines. | Confirmed |
| D5 | Customer-facing sales and consultation features are out of scope. | Confirmed |
| D6 | Tubes are modeled naively: realistic but generalized, for linkage design and simulation (mass, center of mass, side view, rough clearance). No miters, butting or fabrication data. Detailed 3D frame modeling happens afterwards in the builder's CAD. | Confirmed |
| D7 | Construction method doesn't affect version 1; the export is method-agnostic. | Confirmed |
| D8 | CAD export formats: a DXF 2D sketch (points, lines, tube centerlines) and a named-values table (CSV or design table) to drive a parametric master model. | Confirmed |
| D9 | Version 1 frame types: rigid frames, hardtails and general four-bar linkages (single pivot, Horst, split pivot and others), with e-bike support. Six-bars come later. | Confirmed |
| D10 | Version 1 e-bike support: mid-drive motors and gearboxes (outline, mounting bolts, crank axis as origin) and battery mass and position. Rear hub motors, CVTs and hub gears are out of version 1. | Confirmed |
| D11 | Version 1 analysis: kinematics (axle path, leverage ratio, chain growth, pedal kickback), anti-squat and anti-rise, the full rider model (ascending and descending positions, weight distribution, inertia), and the ride-feel report. | Confirmed |
| D12 | The pivot optimizer and the AI design assistant are out of version 1. | Confirmed |
| D13 | Version 1 runs on Linux and macOS with GPUI. Windows comes later, so Windows-only SolidWorks users need another machine to run bikestudio, though they can use the exports. | Confirmed |
| D14 | Version 1 success, in order: (1) accuracy: geometry reproduces published charts, and linkage curves match the reference calculator on synthetic frames and are internally consistent across designs (see S-REF R4); (2) the owner designs and builds a real frame from bikestudio's export. Adoption by outside builders isn't a version 1 criterion. | Confirmed |
| D15 | Distribution: open source and free. Component data is contributed by the community through pull requests. | Confirmed |
| D16 | Licence: MIT OR Apache-2.0, dual-licensed, covering code and data. | Confirmed |
| D17 | Version 1 ride feel shows drivers and sensitivities only (for example, "−1° head tube angle → more stability"). No reference-bike set and no relative placement; that comes later. | Confirmed |
| D18 | Units: millimetres and degrees internally. The UI can display inches, and exports use the unit the user selects. | Confirmed |
| D19 | The tire component is out of version 1; wheel size alone sets wheel and tire diameter. | Confirmed |
| D20 | No wizard in version 1; a new project opens empty (`Project::default()`). | Confirmed |
| D21 | The dimensioned drawing exports as both PDF and SVG. | Confirmed |
| D22 | S-LINK keeps the Newton-Raphson fallback alongside the analytic four-bar solver in version 1. | Confirmed |
| D23 | Version 1 clearance is 2D only. The tire envelope is the outer diameter for the wheel size, using the most common sidewall, from `wheel_size.toml`. No tire width input. | Confirmed |
| D24 | Architecture: a `bike-export` crate owns exports (reads `Project` and `Solved` only); naive tubes live in `bike-geometry`; export libraries are `dxf`, `svg`, `csv`, and `printpdf` or `krilla` (to be evaluated). | Confirmed |

## Conflicts with the current plan

- The plan has no export until Phase 7+ ("CAD and STEP export"). D4 and D8 move a drawing and data export into the core product.
- The plan places the optimizer (Phase 6) and assistant (Phase 6A) before extensions; D12 moves both after version 1.
- The plan positions feel relative to a reference set of bikes; D17 defers the reference set and shows drivers and sensitivities only.
- The plan includes a tire component (D19), a wizard (D20), hub motors, CVTs and hub gears (D10), and Windows in CI (D13); version 1 drops them.

## Specs identified

Build order. Each spec depends only on specs above it.

| # | Spec | Scope | Source |
|---|---|---|---|
| 1 | S-REF | Reference test set: linkage reference tool, 3–5 geometry charts, known answers | D14 |
| 2 | S-MODEL | Workspace, CI, unit newtypes, `Project` types, versioned save and load | Plan Phase 0 |
| 3 | S-UNITS | Inch/mm display and input conversion, export unit setting | D18 |
| 4 | S-GEOM | Pinned/free geometry solver and constraint diagnostics | D3 |
| 5 | S-TUBES | Naive tubes: centerlines, generic diameter and wall, mass estimate | D6 |
| 6 | S-COMPONENTS | Component data and templates: forks, shocks, drive units, batteries | D10, D19 |
| 7 | S-SHELL | Studio shell, side view, undo and redo | D20 |
| 8 | S-EXPORT | PDF and SVG drawing, DXF sketch, named-values table. Rigid and hardtail first; pivots once S-LINK exists | D4, D8, D21 |
| 9 | S-LINK | Linkage graph, analytic four-bar solver, Newton fallback | D9, D22 |
| 10 | S-LINK-EDITOR | Palette, snapping, edit pane, templates, pivot placement, callouts | D3 |
| 11 | S-KINEMATICS | Sag, travel sweep, axle path, leverage ratio, chain growth, pedal kickback | D11 |
| 12 | S-CLEARANCE | 2D tire envelope (wheel-size diameter) against front triangle and drive unit | D23 |
| 13 | S-MASS | Rider model, ascending and descending positions, center of mass, inertia | D11 |
| 14 | S-FORCES | Anti-squat per chain line, anti-rise | D11 |
| 15 | S-FEEL | Feel drivers and sensitivities, no reference set | D17 |

Breakdown approved.
