# S-SHELL — Studio Shell and Side View

**Status:** approved 2026-11-04. **Source:** plan section 1 (`bike-application`, user interface), section 3 (architecture), section 5 (display preferences, themes), design principles 6 and 7, D13, D20, D25 in `../decisions.md`, S-UNITS U3 and U13, S-COMPONENTS C12 and C13. Mockups: `../designer experience images/Specification/`.

## Purpose

The first runnable app. A designer opens bikestudio, creates or opens a `.bikestud` project, sets wheels, travel and geometry in the Specification panel, and sees the bike in the side view, redrawn from `Solved` on every edit, with the diagnostics listed underneath. Undo and redo come with S-LINK-EDITOR, and picking components with a later Components tab.

## Decisions

| # | Decision | Status |
|---|---|---|
| H1 | **Crate:** a new `bike-application` crate, the only one that knows about a user interface (plan section 1), with the binary `bikestudio`. It depends on `gpui-kit`. The plan says `0.8`; crates.io now has `0.9.0` (2026-11-02), so pin `0.9` unless it fails on Linux or macOS. `resolve` stays in `bike-components` (C12); the app calls it. | Confirmed |
| H2 | **Command and reducer** (plan section 3): the interface sends `Command`s: `SetWheel`, `SetTravel`, `PinGeometry`, `FreeGeometry`, `SetFork`, `SetCrank`, and `SetTubes`. `PickComponent` comes with the Components tab. A reducer applies each one to the `Project` and raises a generation counter. The pipeline (`resolve`, `solve_geometry_with`, `solve_tubes_with`) runs again only when the counter changes. No undo stack yet. Changing components will never be an undo step (S-LINK-EDITOR). | Confirmed |
| H3 | **Layout** (mockup `04`, without the **Next: Linkage** button, which isn't built yet): a top bar, the side view, the Specification panel on the right, a bottom tray, and a status bar. The top bar holds the **Bikes** menu, the project name with a saved/unsaved dot, the **Specification · Linkage · Analysis** mode switch, the settings button and the wordmark. Linkage and Analysis are shown but disabled until S-LINK-EDITOR and S-KINEMATICS. The panels are `gpui-component` `dock` and `resizable`. | Confirmed |
| H4 | **Specification panel** (mockups `04`–`12`): **Wheels** front and rear as a segmented choice of the `wheel_size.toml` keys. **Travel** front and rear, always in millimetres (U12). **Geometry**: one row per S-GEOM variable, each with a lock (pinned or free) and a value field. A pinned field holds the designer's value. A free field shows the solved value, dimmed, and a value from the defaults is labelled as such. Typing into a free field pins it. Fields use the S-UNITS parser and the no-drift rule (U7). | Confirmed |
| H5 | **Both seat angles:** the mockups show one "Seat angle" row, but S-GEOM has two independent ones (G4). The panel shows **Effective seat angle** and **Actual seat angle** as separate rows, each pinnable. The effective angle is what the published charts give and all the chart calculations need. | Confirmed |
| H6 | **Frame section** in the panel, below Geometry: material, seat tube length (lockable like geometry) and the three junction offsets (S-TUBES). Per-tube sections go in a collapsed "Tubes" group. Crank length and pedal size go in a "Cranks" group. | Confirmed |
| H7 | **Side view:** a GPUI `canvas` drawn from `Solved` alone: ground line, wheels at their diameters, the fork from its S-COMPONENTS drawing (C13), the frame tubes as centrelines with their diameters, the drive unit outline when one is fitted, and the crank centre marked "0,0 crank axis". The wheelbase is dimensioned. A focused geometry field shows its dimension callout (mockup `07`). For a hardtail, the at-sag and full-travel states can be shown as ghost overlays. Pan with the middle mouse button or space-drag, zoom with the wheel, and **Fit** frames the bike. | Confirmed |
| H8 | **Always drawable** (design principle 6): with nothing pinned, the view draws the default bike and labels it "Side view · preview from defaults", as in the mockups. When the solve fails, the last good drawing stays up, dimmed, with the errors in the tray. | Confirmed |
| H9 | **Bottom tray tabs:** **Diagnostics**, **Curves**, **Feel** and **Assistant**. Diagnostics lists every diagnostic with its severity icon, code and message. Curves, Feel and Assistant show "Coming soon" until S-KINEMATICS, S-FEEL and the post-version 1 assistant (D12) fill them. There's no History tab. Components will be picked from a **Components** tab, which comes in a later spec, not S-SHELL. | Confirmed |
| H10 | **Files** (mockups `01`–`03`, `13`): the Bikes menu has New project (Ctrl+N), Open (Ctrl+O), Change folder (default `~/Bikes`), the five most recent projects, Save (Ctrl+S) and Save as (Ctrl+Shift+S). macOS uses Cmd. New opens `Project::default()` (D20), named `untitled`. Closing the window or opening another project with unsaved changes asks Save, Don't save or Cancel. There's no autosave. Files use `.bikestud` (D25), and a load error shows `LoadError`'s message. | Confirmed |
| H11 | **App settings** (mockup `14`), stored in the platform config directory, never in a project: theme (Dark or Drafting) and the projects folder. The display unit toggle stays in the status bar (U13). The mockup's Assistant section is left out (D12). | Confirmed |
| H12 | **Themes:** Dark ships in S-SHELL. The Drafting theme (plan section 5) draws the same scene with different rendering, so the scene is built once as plain shapes with a style role (frame, wheel, dimension, centreline), and the theme maps roles to strokes and fills. Drafting itself comes in a later spec. | Confirmed |
| H13 | **No frame budget benchmark yet.** Performance is optimised later; S-SHELL adds no benchmark or CI timing check. | Confirmed |
| H14 | **Testing without a window:** the reducer, the panel's view model (what each field shows and whether it's dimmed, pinned or defaulted) and the scene builder are plain Rust and unit tested. GPUI tests (`gpui::TestAppContext`) cover opening, editing and saving a project. CI builds the app on Linux and macOS. | Confirmed |
| H15 | **No new diagnostic codes.** The mockups' `geometry.unpinned`, `suspension.none`, `rider.default` and `linkage.empty` aren't added until it's clear they're needed; the tray shows the codes the solvers already produce. | Confirmed |

## Scope

1. **`bike-application`:** `Command`, reducer, app state (project, path, dirty flag, generation, `Solved`), scene builder, views (top bar, side view, Specification panel, tray, status bar), settings file and file dialogs.
2. **`bike-model`:** nothing new is expected; `Solved` gains `components: ResolvedComponents` if the side view needs it.
3. **CI:** build `bike-application` on both systems. On Linux, CI installs GPUI's system packages.
4. **Out of scope:** undo and redo (S-LINK-EDITOR), picking components (the Components tab), the Linkage and Analysis modes, the contents of the Curves, Feel and Assistant tabs, the Drafting theme (H12), exports (S-EXPORT) and clearance highlighting (S-CLEARANCE).

## Done when

- `cargo run -p bike-application` opens an empty project on Linux and macOS showing the default preview.
- Pinning each geometry value redraws the side view, and the free values update.
- New, Open, Save and Save as round-trip a `.bikestud` file, and unsaved changes prompt before they're lost.
- CI builds and tests `bike-application` on Linux and macOS.

## Open questions

- None.
