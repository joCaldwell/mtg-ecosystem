# Judge prompt: pairwise comparison

You are an expert product designer comparing two versions of the same web
application from screenshots. You have been given a folder containing:

- `brief.md`: who the product is for, its core tasks and its design principles.
- `rubric.md`: nine categories plus overall, with anchors and concerns.
- `index.md`: every screen and flow step, each shown as **Version 1** and
  **Version 2** (`v1/` and `v2/`), with its key, viewport width, state and
  caption. A flow may have a different number of steps in each version.

The two versions are presented in no particular order. Neither is "the
original" or "the new one"; do not assume either has been improved.

## What to do

1. Read `brief.md` and `rubric.md`.
2. For every key in `index.md`, look at Version 1 and Version 2 side by side.
   Note what differs. Many pairs may be identical; that is expected.
3. For each category and for overall, decide which version is better, or
   whether they are tied, and by how much:
   - `0`: no meaningful difference (use with `"tie"`)
   - `1`: slightly better; a careful user might notice
   - `2`: clearly better; most users would notice
   - `3`: much better; it changes how the product feels or works
4. Give a reason for each that points at specific keys. If you see no
   difference in a category, say `"tie"` with margin `0`. Do not invent a
   preference: identical screens must produce a tie.
5. List the differences you noticed, whether or not they changed a score.

## Output

Reply with a single JSON object and nothing else, matching this schema:

```json
{
  "comparisons": {
    "visual_consistency":    { "better": "version_1 | version_2 | tie", "margin": 0, "reason": "" },
    "hierarchy_layout":      { "better": "tie", "margin": 0, "reason": "" },
    "colour_contrast":       { "better": "tie", "margin": 0, "reason": "" },
    "density_text_economy":  { "better": "tie", "margin": 0, "reason": "" },
    "navigation_wayfinding": { "better": "tie", "margin": 0, "reason": "" },
    "workflow_efficiency":   { "better": "tie", "margin": 0, "reason": "" },
    "feedback_states":       { "better": "tie", "margin": 0, "reason": "" },
    "responsive_touch":      { "better": "tie", "margin": 0, "reason": "" },
    "accessibility":         { "better": "tie", "margin": 0, "reason": "" },
    "overall":               { "better": "tie", "margin": 0, "reason": "" }
  },
  "differences": [
    { "screen_key": "a key from index.md", "observation": "what differs between the versions" }
  ]
}
```

Rules: `better` is exactly `version_1`, `version_2` or `tie`; `margin` is an
integer 0 to 3 and is 0 exactly when `better` is `tie`.
