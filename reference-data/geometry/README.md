# Geometry reference charts

Published geometry from Nicolai's official tech sheets (S-REF R1, S-GEOM G10), one TOML file per chart. The S-GEOM tests pin some chart values and check that bikestudio reproduces the others within ±3 mm and ±0.3° (S-REF R6).

Conventions, matching bikestudio's:

- Lengths in millimetres, angles in degrees.
- `head_tube_length` is the chart value. The tests add `lower_headset_stack`, because bikestudio's head tube length includes it (S-GEOM G3).
- `bottom_bracket_drop` is positive when the BB is below the axles. Nicolai prints it as a negative "BB height to axle" (`-75 mm` is a drop of 75).
- Values a chart doesn't give are left out. Where a chart gives a range or a single value for every size, the file's `notes` say which value is used and why.
- A value corrected under S-REF R7 is listed in that size's `corrections`, with the original value and the reason.
- A value that disagrees with the chart's own other values by more than the tolerance is listed in that size's `known_gaps`: which test, which value, the gap (solved minus chart, in mm) and the reason. The tests don't hold it to ±3 mm, but they fail if its gap moves by more than 0.2 mm (S-REF "Changes after approval").
- `seat_tube_length` is centre-to-top along the seat tube axis, and `seat_tube_overhang` is the seat tube above the top tube junction (Nicolai's "Sitzrohrüberstand"). S-TUBES pins both and checks the tubes are placed at those distances (S-TUBES T7).
- `frame_weight` (kg) and `frame_material` are recorded where the chart gives a weight. The S-TUBES tests report it against the estimate; they don't test it.
