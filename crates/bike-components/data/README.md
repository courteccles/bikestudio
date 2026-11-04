# Component library

One TOML file per manufacturer per kind (S-COMPONENTS C2): `forks/`, `shocks/`, `drive_units/`, `batteries/`. Lengths in millimetres, masses in grams, angles in degrees. Every entry cites its source, with the document, its revision or the date it was retrieved, and where in it the values are.

To add a file, list it in `src/library.rs` (`FILES`); the tests parse and validate every listed file.

- **Forks and shocks** are one entry per model and year, with a variant per size. Each variant's id is `make/model/year/variant`, lower case with spaces as hyphens.
- **Drive units and batteries** are one entry per physical unit, in the unit's own coordinates: the crank centre at the origin for a drive unit, the lower end for a battery.
- **Drive unit outlines** come only from a public source drawing (C7). Mark an outline the source gives only roughly with `outline_accuracy = "approximate"`. Never estimate an outline from photos.
- **Centre of mass:** give `centre_of_mass = { x = ..., y = ... }` inside `mass` when the source publishes it; otherwise leave it out and the centroid of the component's drawing is used, and say so in the source.

There are no drive units or batteries yet: Pinion and the motor makers publish no frame interface drawings, so they're entered as custom components (C3, C10).
