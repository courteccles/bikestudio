# S-GEOM chart check: one size that passes, one that fails

**Date:** 2026-11-02. **Chart:** Nicolai G1 29" (factory setup), from Nicolai's official tech sheet ([Google Sheet](https://docs.google.com/spreadsheets/d/e/2PACX-1vQNKldz_0iVoM5d0lhAkD4EWRDZw8XSwgGz0AJOl9HQfAX317oJy77NJXGWP_3JabsGd6xQQatPCaMA/pubhtml), tab "Factory Setup 29\"").

Both bikes come from the same chart, the Nicolai G1 29": **size M passes, size L fails**. They share the same fork, wheels and formulas, so any difference in the result comes from the chart's numbers.

## What the test does

The chart publishes more values than the frame actually needs. With the wheels and fork known, five values fix the frame, and every other value follows from them. So the test:

1. **Pins** 5 chart values: stack, reach, head angle, head tube length and chainstay.
2. **Calculates** everything else from those.
3. **Compares** the calculated values with the ones the chart also publishes: wheelbase and front centre.
4. **Passes** if each one is within ±3 mm of the chart (S-REF R6).

The test is `charts_reproduce_with_head_tube_length_pinned` in `crates/bike-geometry/tests/charts.rs`, and the chart data is in `reference-data/geometry/nicolai-g1-29.toml`. The numbers below come from an independent Python calculation that uses the same formulas. The Rust solver gives the same results to 0.1 mm.

## Coordinates

```
            head tube top T = (reach, stack)
                 \
                  \  head tube (length incl. lower headset stack)
                   \
                    \  fork: axle-to-crown along the steering axis,
                     \       then offset forward, square to it
   rear axle          \
      o-------- BB ----o front axle F
       \   chainstay  /
  ______\____________/______ ground
```

- **Origin:** the crank centre (BB). +X points forward and +Y points up.
- **Head angle (HA):** measured from the horizontal. Going down the steering axis means moving forward by cos HA and down by sin HA for each millimetre.

## Inputs, side by side

| Input | Size M | Size L | Where it comes from |
|---|---|---|---|
| Stack | 646 | 657 | chart (pinned) |
| Reach | 478 | 500 | chart (pinned) |
| Head angle | 63.2° | 63.0° | chart (pinned) |
| Head tube length | 140 + 4 = **144** | 150 + 4 = **154** | chart + lower headset stack (chart says 3–5 mm; 4 used) |
| Chainstay | 454 | 460 | chart (pinned) |
| Fork axle-to-crown | 582 | 582 | chart (582 mm at 170 mm travel) |
| Fork offset | 44 | 44 | chart |
| Wheel radius, front and rear | 370 | 370 | chart tyre diameter 740 mm ÷ 2 |

All lengths are in millimetres.

## The maths, step by step

**Step 1: front axle position.** Start at the head tube top, go down the steering axis by the head tube length plus the axle-to-crown, then forward by the offset, square to the axis:

```
Fx = reach + (head tube + axle-to-crown) × cos HA + offset × sin HA
Fy = stack − (head tube + axle-to-crown) × sin HA + offset × cos HA
```

| | Size M | Size L |
|---|---|---|
| cos HA, sin HA | 0.4509, 0.8926 | 0.4540, 0.8910 |
| head tube + axle-to-crown | 144 + 582 = 726 | 154 + 582 = 736 |
| **Fx** (front centre) | 478 + 726 × 0.4509 + 44 × 0.8926 = **844.6** | 500 + 736 × 0.4540 + 44 × 0.8910 = **873.3** |
| **Fy** | 646 − 726 × 0.8926 + 44 × 0.4509 = **17.8** | 657 − 736 × 0.8910 + 44 × 0.4540 = **21.2** |

**Step 2: BB drop and BB height (G8).** Both tyres stand on flat ground and the wheels are the same size, so the axles sit at the same height. BB drop is the axles' height above the crank centre:

```
BB drop   = Fy − (front radius − rear radius) / 2   = Fy    (equal wheels)
BB height = (front radius + rear radius) / 2 − BB drop
```

| | Size M | Size L |
|---|---|---|
| BB drop | **17.8** | **21.2** |
| BB height | 370 − 17.8 = **352.2** | 370 − 21.2 = **348.8** |

**Step 3: rear axle (Pythagoras).** The chainstay is the straight line from the crank centre to the rear axle, and the rear axle is BB drop above the crank centre:

```
rear centre = √(chainstay² − rear axle height²)
```

| | Size M | Size L |
|---|---|---|
| rear centre | √(454² − 17.8²) = **453.7** | √(460² − 21.2²) = **459.5** |

**Step 4: wheelbase.**

```
wheelbase = front centre + rear centre
```

| | Size M | Size L |
|---|---|---|
| wheelbase | 844.6 + 453.7 = **1298.3** | 873.3 + 459.5 = **1332.9** |

## The comparison

| Checked value | Size M: calculated | M: chart | M: difference | Size L: calculated | L: chart | L: difference |
|---|---|---|---|---|---|---|
| Wheelbase | 1298.3 | 1299 | −0.7 ✅ | 1332.9 | 1325 | **+7.9** ❌ |
| Front centre | 844.6 | 844 | +0.6 ✅ | 873.3 | 865 | **+8.3** ❌ |

The pass rule is |difference| ≤ 3 mm. **Size M passes. Size L fails by about 8 mm on both values.**

## Why size L fails

Front centre (step 1) only uses values the chart publishes: reach, stack, head angle, head tube and fork. No assumption of bikestudio's is involved, apart from the 4 mm lower headset stack, which moves the result by only 1–2 mm. So **for size L, the chart's own numbers don't agree with each other by about 8 mm.** The chart's front centre of 865 would need one of these instead:

- a reach of **491.7** (chart: 500), or
- a head angle of **63.75°** (chart: 63.0°).

The size steps show the same thing. From M to L the chart adds 22 mm of reach, 10 mm of head tube and 11 mm of stack, and the maths says that adds **28.7 mm** of front centre. The chart only adds **21 mm** (844 → 865).

Other evidence that the chart is at fault, not the maths:

- **The Nicolai G1 29" and G1 mullet tabs list identical wheelbase and front centre for every size,** even though their head angles and rear wheels differ. One looks copied from the other.
- **Errors change sign between sizes** (S is 7 mm short, L is 8 mm long). A formula error would push every size the same way.
- **The same formulas reproduce the Nicolai Argon GX within 1 mm** on 4 of its 6 sizes.

The likely explanation is that the chart's head angle is the "factory setup" value, and the G1's mutators (swappable parts) change it per size. The front centre and wheelbase may come from a different mutator setting than the published head angle.

## What this means for the test

The test can't tell a chart error from a solver error by itself. The only way to show that size L's chart is wrong is with this same maths, so "correcting" chart values to match would be circular. That's why I'm asking you how to handle the charts that don't hold together, rather than correcting them myself.

## Decision

Kept at ±3 mm, with option 1: the 45 chart values that don't hold together are recorded as known gaps in the chart files instead of being corrected. Size L's +7.9 mm wheelbase and +8.3 mm front centre are two of them. The tests skip known gaps for the ±3 mm check, but fail if any gap moves by more than 0.2 mm.
