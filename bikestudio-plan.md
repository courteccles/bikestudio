# bikestudio — Architecture and Build Plan

A native desktop design studio, written in Rust, for building a parametric model of a bicycle frame: specification, components, geometry and, for full-suspension bicycles, a functional linkage designed around shock size, leverage ratio, anti-squat, anti-rise and chain growth.

**Goal:** bikestudio is a free, open-source Linux and macOS app where boutique and hobby builders settle rigid, hardtail, or full suspension frame geometry and pivots. It exports a drawing and CAD-ready data for detailed design.

**Version 1 scope** is set by the confirmed decisions in `decisions.md` (D1–D23), which take precedence over this plan. Items marked *post-version 1* are kept for later and aren't built in version 1.

**Guiding principle:** the core is a pure, headless library, and the user interface is a thin layer that reads the solved model and dispatches commands. This gives both maintainability and speed.

---

## 1. Technology Stack

### Workspace layout

| Crate | Responsibility |
|---|---|
| `bike-model` | Data types, unit newtypes, serialization. No math beyond trivial helpers. |
| `bike-geometry` | Geometry constraint solver (pinned/free variables), cockpit contact points, naive tubes (centerlines with generic diameter and wall thickness, for mass, side view and export; not fabrication data). |
| `bike-kinematics` | Linkage motion: axle path, instant center, leverage ratio, chain growth, pedal kickback, rear wheel position at sag. Needs no mass. |
| `bike-mass` | Component and rider masses, ascending and descending rider positions at sag, combined center of mass, weight distribution, moment of inertia. |
| `bike-forces` | Anti-squat (ascending center of mass), anti-rise (descending center of mass). Hub motor drive anti-squat is *post-version 1*. |
| `bike-feel` | Translates geometry, kinematics, mass and forces into ride feel. Adds no new physics. Holds the feel rules as data. |
| `bike-export` | Exports the settled design: a dimensioned side-view drawing (PDF and SVG), a DXF sketch (points, lines, tube centerlines, pivots) and a named-values table (CSV) for parametric CAD models. Reads `Project` and `Solved` only. |
| `bike-optimizer` | *Post-version 1.* Pivot optimization against target curves and feel targets. Re-runs the whole pipeline per candidate. |
| `bike-components` | Component data only: forks, shocks, cockpit parts, drive units (mid-drive motors and gearboxes), batteries. Tires are *post-version 1*. |
| `bike-defaults` | Default values, one TOML file per data structure: `rider.toml` (presets, limb fractions, descending posture), `sag.toml`, `wheel_size.toml`, and `fork.toml` and `cockpit.toml` by travel and wheel size. |
| `bike-assistant` | *Post-version 1.* The design assistant: talks to the Claude API, finds components on the web, turns them into component data, explains designs and proposes edits as `Command`s. Knows no user interface. |
| `bike-application` | User interface. The only crate that knows a user interface exists. |

Dependency order (each crate reads only from the layers below it):

```
bike-model
   ↑
bike-geometry        static frame points, cockpit contact points
   ↑
bike-kinematics      linkage motion, rear wheel position at sag
   ↑
bike-mass            rider positions placed at sag, combined center of mass
   ↑
bike-forces          anti-squat, anti-rise, drive anti-squat
   ↑
bike-feel            reads everything above
   ↑
bike-optimizer       re-runs the whole pipeline per candidate
```

`bike-export` depends on `bike-model` and the solve pipeline, and is called by `bike-application`. `bike-components` and `bike-defaults` depend only on `bike-model`. `bike-assistant` depends on `bike-model`, `bike-components` and the solve pipeline, and is called by `bike-application`; no core crate depends on it, so the app works fully without it. A resolution step in `bike-application` turns every selected `ComponentReference` into concrete values, stands in default fork and cockpit dimensions and masses from `bike-defaults` where no component is selected, and fills every missing rider and sag field from its defaults before solving. Inputs with no default (travel, wheel size, pivot positions, geometry pins) may still be missing, so each core crate computes what its inputs allow and reports the rest as diagnostics.

One crate per concern keeps compile times low, enforces boundaries, and lets contributors work on the math without touching the user interface.

### Crates

- **User interface: `gpui-kit`** on top of GPUI (Zed's GPU-accelerated framework). Views render from the solved model each frame they are notified, so there is no stale user interface state, and GPUI's `canvas` element handles the custom side view. One dependency (`gpui-kit = "0.8"`) pins the matching GPUI release and re-exports `gpui-component`, which supplies the studio shell: `dock` and `resizable` panels, `form`, `input`, `slider`, `select`, `table` and `tree`. Its `chart` and `plot` modules cover the curves (leverage ratio, anti-squat, chain growth); fall back to a custom `canvas` plot only if they fall short (for example, live pivot-drag performance).
- **Math: `glam`** for two-dimensional vectors. `nalgebra` only for small linear solves (Newton steps).
- **Units:** newtypes (`Millimeters(f64)`, `Degrees(f64)`) rather than `uom`. They're lighter and still prevent unit-mixing bugs. Everything is stored and solved in millimetres and degrees; the user interface can display and accept inches, and exports use the unit the user selects.
- **Export:** `dxf` for the DXF sketch, `svg` for the drawing, a PDF writer (`printpdf` or `krilla`, to be evaluated), and `csv` for the named-values table.
- **Serialization: `serde` + `serde_json`** for `.bikestud` files, with a `schema_version` and migrations from day one.
- **Component, rider and reference data:** TOML files, so values can be tweaked without touching Rust and the community can contribute through pull requests. Every entry cites its source.
- **Default presets:** each preset is an enum named `<StructName>Defaults` (for example, `RiderDefaults` for `Rider`), defined in `bike-model`. Its values live in `bike-defaults/<struct>.toml` (for example, `bike-defaults/rider.toml`) and are embedded at compile time with `include_str!`. Tests check that every file parses and covers every enum variant, and that a project whose wheel size key is missing from `wheel_size.toml` still loads with a diagnostic. Move components to `rusqlite` only if the library grows large.
- **Concurrency: `rayon`** for parallel sweeps (for example, anti-squat per gear). A worker thread with `crossbeam-channel` runs the optimizer.
- **Optimization:** the `cmaes` crate or `argmin`. Nelder-Mead is simple to hand-roll.
- **Testing:** `approx` (float comparisons), `proptest` (solver invariants), `insta` (curve snapshots), `criterion` (performance budgets).
- **Assistant (post-version 1):** the Claude API (Messages API with tool use) over `reqwest`, with web search and web fetch as server-side tools. Defaults to the latest Claude model, configurable. The API key comes from the user's settings or the `ANTHROPIC_API_KEY` environment variable and is stored in the operating system keychain (`keyring`), never in a `.bikestud` file.
- **Platforms:** Linux and macOS in version 1; Windows later.
- **Licence:** MIT OR Apache-2.0, dual-licensed, covering code and data.
- **Later:** a three-dimensional view, either through GPUI's renderer or an embedded `wgpu` surface (to be evaluated). Finite element analysis sits behind a trait so a solver can be swapped in.

---

## 2. Core Data Structures

```rust
// bike-model
// Project::default() is an empty project: nothing is required to open one
pub struct Project {
    pub schema_version: u32,
    pub specification: Specification,
    pub components: Components,
    pub geometry: GeometryInputs,
    pub front_travel: Option<Millimeters>,       // None = rigid fork; a selected fork must match
    pub rear_suspension: Option<RearSuspension>, // None = rigid rear; created when rear travel is entered
    pub sag: Sag,                                // fork and shock sag, defaults from travel
    pub rider: Rider,                            // optional measurements, defaults fill gaps
    pub feel_target: Option<FeelTarget>,         // post-version 1, used by the optimizer
}

pub struct Rider {
    pub mass_with_gear: Option<Kilograms>,
    pub height: Option<Millimeters>,
    pub wingspan: Option<Millimeters>,
    pub inseam: Option<Millimeters>,
    pub defaults: RiderDefaults,         // used for any field left as None
}

pub enum RiderDefaults { Small, Medium, Large } // values in bike-defaults/rider.toml

pub struct Specification {
    pub front_wheel: Option<WheelSize>,
    pub rear_wheel: Option<WheelSize>,     // supports mullets
    pub drive: DriveSystem,
}

// The wheel with its tire. A key into bike-defaults/wheel_size.toml (for example "29"),
// which gives each size its outer diameter, so a new standard is one new entry
pub struct WheelSize(pub String);

pub struct DriveSystem {
    pub unit: Option<DriveUnit>,                     // None = bare bottom bracket
    pub rear: Option<RearDrive>,
    pub idler: Option<Idler>,
    pub battery: Option<BatterySetup>,               // battery + mount position
}

// The drive unit as placed in this design
pub struct DriveUnit {
    pub component: ComponentReference<DriveUnitComponent>,
    pub rotation: Degrees, // about the crank axis; affects center of mass, clearance and rendering only
}

// Component data, defined in the unit's own coordinates, centered on the crank axis, which is also the output axis
pub struct DriveUnitComponent {
    pub outline: KeepOut,                  // the unit's casing, for clearance checks and rendering
    pub mounts: Vec<Vector2Millimeters>,   // bolt positions the frame must provide
    pub output: Option<Sprocket>,          // fixed output sprocket for gearboxes, on the crank axis
    pub motor: Option<MotorSpecification>, // electric assist
    pub ratios: Option<Ratios>,            // internal transmission
    pub mass: MassProperties,              // center of mass relative to the crank axis
}

pub enum Ratios { Discrete(Vec<f64>) } // Continuous (CVT) is post-version 1

pub enum RearDrive {
    Cassette(Vec<u8>),                    // derailleur
    SingleCog(u8),                        // gearbox or single speed
    // HubGear and HubMotor are post-version 1
}

// All optional: selections add clearance checks, rendering and exact dimensions,
// and default values from bike-defaults stand in until then
pub struct Components {
    pub fork: Option<ComponentReference<Fork>>,       // axle-to-crown, offset, travel
    pub shock: Option<ComponentReference<Shock>>,     // eye-to-eye, stroke, mounts
    // Tires are post-version 1; WheelSize gives the 2D tire envelope
    pub cockpit: Option<Cockpit>,
}

pub struct Cockpit {                    // places the hands, saddle and pedals
    pub stem_length: Option<Millimeters>,
    pub bar_rise: Option<Millimeters>,
    pub bar_width: Option<Millimeters>,
    pub spacers: Option<Millimeters>,
    pub crank_length: Option<Millimeters>,
    pub saddle_setback: Option<Millimeters>,
}                                      // None fields use bike-defaults/cockpit.toml

pub enum ComponentReference<T> { Library(ComponentIdentifier), Custom(T) } // Library = bike-components

// Each geometry variable is pinned by the user or left free for the solver.
// Pinned values are static: the frame as built, unweighted, as geometry charts publish it
pub struct GeometryInputs { pub variables: EnumMap<GeometryVariable, Parameter> }
pub enum Parameter { Pinned(f64), Free }
pub enum GeometryVariable {
    Stack, Reach, HeadTubeAngle, SeatTubeAngle, BottomBracketDrop,
    ChainstayLength, Wheelbase, BottomBracketHeight,
}

pub struct RearSuspension {
    pub travel: Millimeters,  // rear wheel travel
    pub linkage: Linkage,     // may be empty until pivots are placed
}

pub struct Sag {
    pub fork: Option<f64>,  // fraction of front travel; None = default from front travel; unused on rigid frames
    pub shock: Option<f64>, // fraction of shock stroke; None = default from rear travel; unused on hardtails
    pub defaults: SagDefaults, // used for any sag left as None
}

pub enum SagDefaults { Standard } // values in bike-defaults/sag.toml, keyed by travel

// Linkage coordinates: origin at the crank axis (the bottom bracket, or the output
// crank of a gearbox or motor), +X toward the front wheel, +Y up, frame static and unweighted
pub struct Linkage {
    pub anchors: Vec<Anchor>,
    pub links: Vec<Link>,
    pub pins: Vec<Pin>,
    pub shock: ShockMounting,
    pub axle: PivotIdentifier,     // the rear axle is a pivot; its path is the axle path
    pub brake_on: LinkIdentifier,
}

// Fixed relative to the origin: where links and the shock attach to the front triangle
pub struct Anchor {
    pub id: AnchorIdentifier,
    pub position: Vector2Millimeters,
}

// A point on a link; its position comes from the solve
pub struct Pivot { pub id: PivotIdentifier }

// Any rigid body with two or more pivots. It behaves as a rocker when one of its
// pivots is pinned to an anchor; nothing else distinguishes the two
pub struct Link {
    pub id: LinkIdentifier,
    pub pivots: Vec<PivotIdentifier>,
    pub geometry: Vec<Measurement>, // spatial relationship between its own pivots
}

pub struct Measurement {
    pub kind: MeasurementKind,
    pub value: Parameter, // Pinned or Free
}

pub enum MeasurementKind {
    Distance { a: PivotIdentifier, b: PivotIdentifier },
    Angle { vertex: PivotIdentifier, a: PivotIdentifier, b: PivotIdentifier },
}

// Joins pivots to each other or to an anchor: 2–3 targets, at most one anchor
pub struct Pin {
    pub id: PinIdentifier,
    pub joins: Vec<PinTarget>,
}

pub enum PinTarget { Pivot(PivotIdentifier), Anchor(AnchorIdentifier) }

// Each eye pins to a pivot or an anchor; eye-to-eye and stroke come from the shock component
pub struct ShockMounting {
    pub eye_a: PinTarget,
    pub eye_b: PinTarget,
}

// Outputs: never saved, always recomputed
// Every result is optional because the model may be incomplete while the designer works
pub struct Diagnostic { pub severity: Severity, pub code: String, pub message: String }
pub enum Severity { Info, Warning, Error }
pub struct Curve { pub points: Vec<(f64, f64)> }  // (travel, value); shared by every result field
pub struct Solved {
    pub geometry: Option<SolvedGeometry>, // every GeometryVariable static and at sag, plus points
    pub mass: Option<MassResult>,         // combined center of mass, weight split, inertia
    pub kinematics: Option<KinematicsResult>,
    pub forces: Option<ForcesResult>,
    pub feel: Option<FeelReport>,
    pub diagnostics: Vec<Diagnostic>,     // missing inputs, over-constrained, clearance hits...
}

pub struct KinematicsResult {
    pub steps: Vec<KinematicsStep>,          // per travel step: axle, instant center, shock length
    pub leverage: Curve,
    pub chain_growth: Curve,
    pub pedal_kickback: Curve,
    pub sag: KinematicsStep,                 // linkage state at shock sag
}

pub struct ForcesResult {                    // bike-forces
    pub anti_squat: Vec<(ChainLine, Curve)>, // one curve per distinct chain line
    pub anti_rise: Curve,
}

pub struct FeelReport { pub axes: Vec<FeelAxis> }

pub struct FeelAxis {
    pub axis: FeelKind,                  // Stability, Agility, Climbing...
    // position relative to a reference set is post-version 1
    pub drivers: Vec<Driver>,            // which metrics pushed it, by how much
    pub sensitivities: Vec<Sensitivity>, // for example: −1° head tube angle → +0.3 stability
}

pub struct MassResult {
    pub bicycle: MassProperties,
    pub ascending: RiderPositionResult,  // seated: saddle, pedals, grips
    pub descending: RiderPositionResult, // standing: cranks level, grips
}

pub struct RiderPositionResult {
    pub rider_center_of_mass: Vector2Millimeters,
    pub system_center_of_mass: Vector2Millimeters, // rider + bicycle
    pub weight_split: WeightSplit,                 // front/rear wheel, hands/feet
    pub contact_points: ContactPoints,             // drawn as a ghost rider in the side view
}
```

### Design principles

1. **Inputs and outputs are separate types.** `Project` is saved and `Solved` is drawn. Nothing derived is ever stored.
2. **Pinned versus Free per variable.** Wheelbase and bottom bracket height depend on other dimensions, so the user pins what they care about and the solver resolves the rest, flagging over- and under-constrained sets.
3. **Generic linkage graph.** Anchors, links, pins and the shock form one graph. Templates only generate starting layouts in the linkage editor and are not stored, so new linkage types need no new solver code.
4. **Pin static, compute at sag.** Pinned geometry describes the frame as built, unweighted, so it matches published geometry charts and doesn't move when sag changes. Solved geometry shows every variable both static and at sag. Mass, forces and feel all use the bicycle at fork sag and, for full-suspension bicycles, shock sag, since that is what riders feel. Curves such as leverage ratio and anti-squat still span the full travel.
5. **One drive unit per physical unit.** An integrated motor and gearbox is one body, one mass and one keep-out volume, all centered on the crank axis.
6. **The model may be incomplete.** The designer fills in the specifics as the design develops. A missing input never blocks the solve: whatever can be computed is shown, and the diagnostics list what is still needed.
7. **Components never block the design.** Travel and wheel size are enough to start designing. Selecting a fork, shock or cockpit adds clearance checks, rendering and exact dimensions, but nothing requires them.

---

## 3. App Architecture

```
User interface ──Command──▶ Reducer ──▶ Project (undo stack, Arc snapshots)
                                           │
                                           ▼
                                    Core solve (synchronous, microseconds)
                                           │
                                           ▼
User interface ◀──── draws ───────── Solved (cached by generation counter)

Optimizer / heavy sweeps ──▶ worker thread ──channel──▶ user interface
```

- The user interface emits `Command` enums: `SetParameter`, `MovePivot`, `PickComponent`, and so on. The linkage editor's commands are listed in section 5.
- The reducer applies a command and pushes the previous state onto the undo stack, so undo/redo comes for free.
- Recomputation runs only when the model's generation counter changes.
- **New projects** open empty (`Project::default()`); there is no wizard.
- **Performance budget:** a pivot drag updates the side view, every curve and the feel panel within 4 milliseconds. Enforce this with a `criterion` benchmark in continuous integration.

---

## 4. Kinematics, Mass and Ride Feel Notes

### Sag

- Fork sag and shock sag are set by the designer and stored in `Project.sag`.
- Any sag left unset defaults from the selected `SagDefaults` preset, a recommendation table keyed by travel in `bike-defaults/sag.toml`. For example, at 160 millimeters of travel the table recommends 20–25% for the fork and 25–30% for the shock. The default is the middle of the recommended range.
- The interface shows the recommended range next to each sag value and labels defaulted values, the same way it labels defaulted rider fields.
- Fork sag is a fraction of front travel. Shock sag is a fraction of shock stroke; with no shock selected, that is the stroke the linkage needs to reach the rear travel. `bike-kinematics` converts shock sag to a rear wheel position.

### Kinematics and forces

Motion (axle path, leverage ratio, chain growth, pedal kickback, rear wheel position at sag) lives in `bike-kinematics`. Anti-squat, anti-rise and drive anti-squat need the center of mass, so they live in `bike-forces`.

- **Solver:** sweep the rear wheel through its travel. Use analytic loop closure for four-bars and Newton-Raphson as the general fallback.
- **Leverage ratio:** d(wheel travel) / d(shock stroke). The linkage determines the shock length and stroke needed for the rear travel; when a shock is selected, check that its eye-to-eye and stroke match.
- **Anti-squat:** intersect the chain force line (from the chainring/cog tangent) with the instant center line, relative to the combined bicycle and rider center of mass. A cassette gives one curve per gear combination. A gearbox gives one chain line and so one curve, because anti-squat percentage is geometric and the internal ratio drops out.
- **Hub motor (post-version 1):** drive force acts at the hub, so its anti-squat comes from the motor's torque reaction through the link it is fixed to, calculated the same way as anti-rise.
- **Anti-rise:** draw the brake line from the rear contact patch through the instant center, or through the floating link for seatstay-mounted brakes.
- **Chain growth / pedal kickback:** track the change in chain length across travel. Use the actual chain contact point for idlers.
- **Clearance check:** the only check is the wheels and tires against the front triangle, including the drive unit outline (see section 5). Everything else waits for three-dimensional modeling.

### Mass

- Each component and frame section carries a mass and center of mass position. Frame mass is estimated from tube volume and material density.
- **Rider inputs are optional:** mass with gear, height, wingspan and inseam. Any missing field comes from the selected `RiderDefaults` preset in `bike-defaults/rider.toml`, with a cited source for each value. Users pick a preset as a starting point and can override any value. Missing fields are scaled proportionally from the fields provided (for example, inseam and wingspan from height). The interface labels every defaulted value.
- **Two riding positions:** ascending (seated, contact at saddle, pedals and grips) and descending (standing, cranks level, contact at pedals and grips, hips back and low).
- **Segmented body model:** limb mass and length fractions are defaults taken from a published anthropometric table and stored in `bike-defaults/rider.toml`. Limb lengths are interpolated from the rider's measurements using those fractions. Contact points come from the geometry and cockpit; joint angles are solved as a small chain problem, with wingspan setting arm segments and inseam setting leg segments. Descending posture parameters (hip height and rearward offset) live in the same file.
- **Which position feeds what:** ascending drives anti-squat, pedaling efficiency and seated climbing. Descending drives anti-rise, braking composure, stability, cornering balance and agility.
- Outputs: combined center of mass, front and rear weight distribution, sprung and unsprung mass, pitch moment of inertia.
- Electric bicycles: drive unit mass, battery mass and battery position move the center of mass. Rear hub motors and hub gears (unsprung mass) are post-version 1.

### Ride feel

- `bike-feel` reads solved outputs and translates them into feel axes. It never adds new physics.
- Version 1 shows each axis's drivers and sensitivities only. Placing a design relative to a reference set of bicycles is post-version 1; feel is never an absolute score.
- Every axis lists its drivers and sensitivities, so the user sees why the bicycle feels that way and what to change.
- Mapping rules are domain data in the `bike-feel` crate, each with a cited rationale and validated by tests. They are modeling inputs, separate from the application's design-language protections.
- The application states that feel results are informed heuristics, calibrated over time by building and riding real bicycles.

Point values in this table are read at sag; curves span the full travel.

| Feel axis | Driven by |
|---|---|
| Stability at speed | Wheelbase, head tube angle, trail, system mass |
| Agility | Wheelbase, chainstay length, pitch moment of inertia, trail |
| Seated climbing | Effective seat tube angle, front wheel load, anti-squat |
| Pedaling efficiency | Anti-squat across the gear range, chain growth |
| Small-bump plushness | Starting leverage ratio, rearward axle path |
| Support | Mid-stroke leverage ratio, progression |
| Bottom-out resistance | End-stroke progression |
| Braking composure | Anti-rise |
| Cornering balance | Front center to chainstay ratio, bottom bracket height, weight distribution |
| Pedal feedback | Pedal kickback |

---

## 5. Linkage Model and Editor

**Purpose:** work out the mechanical linkage, in two dimensions, so it hits the curves the designer wants. Once the pivot positions are settled, three-dimensional modeling of tubes and links begins around them with real clearance in mind (Phase 7+).

### Model

- **Origin:** the crank axis, which is the bottom bracket, or the output crank when a gearbox or motor is fitted. Every coordinate is X/Y in millimeters relative to it, with +X toward the front wheel and +Y up, in the static unweighted frame.
- **Anchors** are fixed relative to the origin. They are where links and the shock attach to the front triangle.
- **Links** are rigid bodies with two or more pivots, in any layout: two-pivot bars, triangles, L shapes, four or more pivots. Their geometry is a set of distances and angles between their own pivots, each `Pinned` or `Free`.
- **Rockers are not a separate type.** A link behaves as a rocker when one of its pivots is pinned to an anchor.
- **Pins** join pivots to each other or to an anchor: 2–3 targets per pin, at most one of them an anchor. A pin with more is a diagnostic error.
- **The shock** attaches by its two eyes, each pinned to a pivot or an anchor.
- **The rear axle** is a pivot on the link that carries it, so the axle path is that pivot's path. A split pivot is not a special case: it is a pin at the axle position.
- **Rigid shape:** a link with n pivots needs 2n − 3 measurements to hold its shape (1 for two pivots, 3 for a triangle, 2 more for each further pivot). The solver reports links that are missing measurements or have conflicting ones, as it does for geometry.
- **Mechanism status:** the solver reports whether the linkage has exactly one degree of freedom (it moves with the shock), is locked, or has too many degrees of freedom.

### Editor

- **Always available.** Until the geometry places the rear axle, the axle pivot floats like any other pivot and diagnostics list what is missing. Once the axle position is solved, the axle pivot snaps to it.
- **Palette:** Anchor, Link, Rocker (a link with an anchor already pinned) and Shock.
  - **Click and release** drops the part loosely in the design area: empty space near the middle of the view, unpinned, not overlapping other parts.
  - **Drag** places the part where it is released.
  - A new link arrives with two pivots and one distance. A new shock arrives at its eye-to-eye length, or a default when no shock is selected.
- **Snapping:** dragging a pivot near another pivot or an anchor highlights the target. Dropping creates a pin, or joins an existing pin that has room. Dragging a pivot off a pin breaks it.
- **Edit pane:** right-click a link and choose **Edit** to open a small pane around the part. It changes the pivot layout (adding, removing and repositioning pivots, and the distances and angles between them) but not the outline. It shows whether the shape is fully defined and lists missing or conflicting measurements. Closing the pane applies the changes as one undo step; Escape cancels them.
- **Templates** (Single Pivot, Horst, Split Pivot and so on) drop a starting layout into the same editor.
- **Nothing blocks editing.** Links with missing or conflicting measurements are highlighted, each problem appears in diagnostics, and the curves appear as soon as the mechanism solves.
- **Commands:** `AddAnchor`, `AddLink`, `AddPivot`, `RemovePivot`, `CreatePin`, `BreakPin`, `MovePivot`, `SetMeasurement`, `RemoveLink`. `MovePivot` states whether its position is absolute or relative. Each command is one undo step, and a drag merges into one.

### Pivot placement

- **Inspector:** selecting a pivot or anchor shows editable X and Y in millimeters. Fields update live while dragging, and a typed value moves the point as soon as it is committed. Fields accept expressions such as `412.5`, `+2` (nudge by 2 millimeters) or `400/2`.
- **Nudging:** arrow keys nudge the selected pivot by the current increment. While a pivot is selected, `[` and `]` step the increment down and up through 0.1, 0.5, 1 and 10 millimeters. Each nudge is one undo step.
- **Readout:** next to the cursor, showing X, Y and the selected increment.
- **Pinned measurements hold during a drag,** so a pivot slides along the arc or line they allow.

### Dimensions in the side view

- The selected pivot shows dimension callouts: X and Y from the origin, plus distances to the pivots it is linked to. The selected link shows its measurements.
- Double-clicking a callout edits it in place.
- **Dimension filter:** a control in the side panel chooses which callouts appear, by type (pivot X/Y, link measurements, distances between pivots, frame geometry, shock eye-to-eye and stroke), by scope (selected pivot, selected link, everything), and whether locked measurements are always shown.

### Outlines

- Each link is drawn with a simple outline generated from its pivots: a rounded shape wrapping them, offset by a radius. A two-pivot link draws as a capsule. Pivots, pins and anchors have their own markers.
- Outlines are representational only. They are not editable, not stored, and not used for clearance.

### Clearance

- **The only check** is the wheels and tires against the front triangle, and the front triangle includes the drive unit outline. It is two-dimensional: the tire envelope is the outer diameter for the wheel size, using the most common sidewall, from `wheel_size.toml`.
  - **Rear:** the rear tire envelope follows the axle path through the full travel.
  - **Front:** the front tire moves through the fork travel along the steering axis, to full compression.
- The shock and links are not checked in two dimensions, because they sit beside the tire and their real clearance needs three dimensions.
- Contacts are highlighted in the side view at the point of travel where they happen, and reported as `Warning` diagnostics.

### Display preferences

- The dimension filter and the outline radius are display preferences, not model data. They are not saved in `Project`, do not affect `Solved`, and do not create undo steps.
- **Theme:** an app setting (Settings › Appearance), stored with the user's settings rather than in a `.bikestud` file.
  - **Dark:** the default. The side view is a dark grid with shaded parts.
  - **Drafting:** the side view, under the trays too, becomes an engineer's hand drafting: pencil on vellum with a light grid, linework only with no fills, ruled lines that run slightly past their ends, line weight by importance (frame and fork, then wheels, then dimension and centre lines), and upper-case lettering between faint guide lines. The rest of the window keeps the dark chrome.
  - Both themes draw the same scene from `Solved`; the theme only changes how it is rendered.

---

## 6. Design Assistant (post-version 1)

*Not built in version 1 (D12). Kept here for later.*

**Goal:** a designer asks for a component by name ("RockShox Zeb Ultimate 170 29") and gets a usable, rendered component in the model, and can ask the assistant for design help that is grounded in the solved model rather than guesswork.

The assistant is a client of the same architecture as the user interface: it reads `Project` and `Solved`, and it changes the design only by proposing `Command`s. It never edits state directly.

### What it can do

| Capability | How |
|---|---|
| Find a component | Web search and fetch of manufacturer pages, spec sheets and PDFs. Manufacturer sources first, retailers and reviews only to fill gaps. |
| Extract its data | Structured output into the component's own type (`Fork`, `Shock`, `Tire`, `DriveUnitComponent`...), one field at a time, each with its source URL and quoted text. |
| Recreate it | Build the outline and keep-out from the extracted dimensions using a parametric template per component type (see below). |
| Help with the design | Explain results ("why is anti-squat 140% at sag?"), suggest changes with predicted effects, and answer how-to questions about the app. |
| Change the design | Propose `Command`s (`PickComponent`, `SetParameter`, `MovePivot`...) that the user previews and accepts. |

### Component pipeline

```
request ─▶ search + fetch ─▶ extract (typed, cited) ─▶ validate ─▶ review card ─▶ accept
                                                          │                         │
                                                          ▼                         ▼
                                              diagnostics, missing fields   user library + PickComponent
```

1. **Extract into typed data, not drawings.** The model fills a JSON schema generated from the component's Rust type (`schemars`). Every value carries a source URL, the quoted text it came from and a confidence. Values the sources don't state stay empty, never guessed.
2. **Validate in Rust.** Units normalized (inches, millimeters, grams), ranges checked (eye-to-eye 165–250 millimeters, offset 37–56 millimeters...), cross-checks (stroke against travel and eye-to-eye standard, a fork's travel against `Project.front_travel`). Failures become diagnostics on the review card.
3. **Render from parametric templates.** Each component type has a template in `bike-components` that draws its side-view outline and keep-out from its dimensions: a fork from axle-to-crown, offset, stanchion diameter and travel; a shock from eye-to-eye, stroke, body and reservoir size; a drive unit from its mounting points and casing dimensions. The assistant never draws freehand; the same template draws library and scraped parts. Where a manufacturer publishes a drawing (a drive unit's bolt pattern, for example), its dimensions feed the template.
4. **Review before use.** The review card shows the rendered component, every value with its source and confidence, and what's missing. Accepting saves it to the user's component library (a TOML file in the same format as `bike-components`, marked `source = "assistant"` with the date and URLs) and optionally picks it. Users can submit good entries upstream through a pull request.
5. **Cache.** Accepted components are reused without another search; a refresh re-checks the sources.

### Design and app expertise

- **Grounded in the solver.** The assistant gets tools to read the project, read `Solved` (geometry, curves, mass, forces, feel with drivers and sensitivities, diagnostics), and run what-if solves on a copy of the project. Numbers it states come from these tools, never from its own arithmetic.
- **Bike design knowledge:** the model's own knowledge, steered by a system prompt written with the `bike-feel` rules and their rationales, and the `bike-defaults` tables. It cites when it relies on these.
- **App knowledge:** the user guide and the `Command` reference are bundled with the app and given to the assistant, so how-to answers match the version the user is running.
- **Proposals, not actions.** Suggested edits arrive as a `Command` batch shown as a diff and a ghost overlay on the side view and curves. Accepting applies them as one undo step.

### Boundaries

- **Optional and offline-safe.** No key or no network means no assistant; everything else works.
- **Respect sources.** Honor `robots.txt` and site terms, fetch only the pages needed, and store extracted values and citations, not copied pages or images.
- **Untrusted web content.** Fetched pages are data. The assistant cannot run commands found in them, and every change still needs the user's acceptance.
- **Privacy.** The user sees what is sent (project data and their messages). Nothing is sent until the assistant is enabled.
- **Cost.** Show token use per conversation; cache prompts (system prompt, user guide) to keep cost down.

### Testing

- Extraction tests against saved copies of real spec pages with known answers (`insta` snapshots), so model or prompt changes show up as diffs.
- Validation and template rendering are pure Rust and tested like the rest of the core.
- An evaluation set of design questions with expected answers grounded in `Solved`, run before changing the model or system prompt.

---

## 7. Version 1 Specs

Version 1 is built as small, separately testable specs, in this order. Each spec depends only on specs above it and gets its own spec file. Decision numbers refer to `decisions.md`.

| # | Spec | Scope | Done when |
|---|---|---|---|
| 1 | S-REF | Reference test set: 3–5 published geometry charts, 6 synthetic linkage frames, and reference calculator answers | Known answers are recorded as test fixtures |
| 2 | S-MODEL | Workspace, CI (fmt, clippy, tests on Linux and macOS), unit newtypes, `Project` types, versioned save and load | An empty and a filled-in project both round-trip to JSON |
| 3 | S-UNITS | Inch and millimetre display and input, export unit setting | Values round-trip between units without drift |
| 4 | S-GEOM | Pinned/free geometry solver and constraint diagnostics | S-REF geometry charts reproduce within ±3 mm and ±0.3° (R6) |
| 5 | S-TUBES | Naive tubes: centerlines, generic diameter and wall thickness, mass estimate | Every frame type yields tube centerlines and a mass estimate |
| 6 | S-COMPONENTS | Component data and templates: forks, shocks, drive units, batteries | Every library entry parses, cites a source and renders |
| 7 | S-SHELL | Studio shell, side view, undo and redo; new projects open empty | Every edit redraws within the frame budget |
| 8 | S-EXPORT | PDF and SVG drawing, DXF sketch, CSV named-values table. Rigid and hardtail first; pivots once S-LINK exists | A rigid frame and a hardtail export and open correctly in CAD |
| 9 | S-LINK | Anchor, link and pin graph; analytic four-bar solver with Newton-Raphson fallback | Single-pivot, Horst and split-pivot layouts solve; invalid ones report diagnostics |
| 10 | S-LINK-EDITOR | Palette, snapping, edit pane, templates, pivot placement, dimension callouts and filter, outlines | Dragging a pivot updates every curve live |
| 11 | S-KINEMATICS | Sag, travel sweep, axle path, leverage ratio, chain growth, pedal kickback | Curves match the S-REF reference calculator answers |
| 12 | S-CLEARANCE | 2D tire envelope against the front triangle and drive unit outline | Contacts are highlighted at the travel where they occur |
| 13 | S-MASS | Rider model, ascending and descending positions, combined center of mass, weight distribution, inertia | A rider with no measurements gets a complete result from defaults |
| 14 | S-FORCES | Anti-squat per chain line, anti-rise | Curves match the S-REF reference calculator answers |
| 15 | S-FEEL | Feel drivers and sensitivities, no reference set | Every axis lists its drivers and sensitivities |

**Version 1 is done when** (D14): geometry matches published charts and curves match the S-REF reference calculator answers, and the owner designs and builds a real frame from bikestudio's export.

### Post-version 1
- Windows support
- Six-bar templates
- Pivot optimizer
- Design assistant
- Ride feel placed against a reference set of bicycles
- Tire components and tire width clearance
- Rear hub motors, hub gears and continuously variable transmissions
- Three-dimensional tube and freeform (metal three-dimensional-printable) frame modeling
- Structural finite element analysis
- STEP file export
