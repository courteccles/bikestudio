# S-MODEL — Workspace, Types and Save/Load

**Status:** in interview. **Source:** plan section 1 and 2, D13, D15, D16, D18, D25 in `../decisions.md`.

## Purpose

Set up the repository, workspace and CI, and define the `Project` types and the versioned save and load that every later spec builds on.

## Decisions

| # | Decision | Status |
|---|---|---|
| M1 | Hosting: git on GitHub, CI on GitHub Actions. Local repository created on branch `main`; no GitHub repository created or pushed yet. | Confirmed |
| M2 | One `.bikestud` file per project, pretty-printed JSON. Users organize projects in ordinary folders (for example `~/Bikes/Customers`), as the mockups show. | Confirmed |
| M3 | The "Next: Linkage →" button in the mockups is tab navigation, not a wizard (consistent with D20). | Confirmed |

## Open questions

- (next interview question)
