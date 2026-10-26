#!/usr/bin/env node
// Pairs two runs capture by capture and reports which images and metrics
// differ. Two runs of the same app and seed should be identical; whatever
// differs is noise the loop must not mistake for a change.
//
//   node compare.mjs <runA> <runB> [--json]

import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

const [a, b] = process.argv.slice(2).filter((x) => !x.startsWith("--"));
if (!a || !b) {
  console.error("usage: node compare.mjs <runA> <runB> [--json]");
  process.exit(2);
}
const hash = (f) => (existsSync(f) ? createHash("sha256").update(readFileSync(f)).digest("hex") : null);
const load = (run, name) => JSON.parse(readFileSync(join(run, name), "utf8"));

function images(run) {
  const m = load(run, "manifest.json");
  const out = new Map();
  for (const s of m.screens) {
    out.set(s.files.viewport, s.key);
    if (s.files.full) out.set(s.files.full, `${s.key} (full page)`);
  }
  for (const f of m.flows) for (const s of f.steps) out.set(s.file, s.key);
  return out;
}

const A = images(a), B = images(b);
const onlyA = [...A.keys()].filter((f) => !B.has(f));
const onlyB = [...B.keys()].filter((f) => !A.has(f));
const differing = [];
let same = 0;
for (const f of A.keys()) {
  if (!B.has(f)) continue;
  if (hash(join(a, f)) === hash(join(b, f))) same++;
  else differing.push(A.get(f));
}

// Metrics that should not move between identical runs.
const ma = load(a, "metrics.json"), mb = load(b, "metrics.json");
const metricDiffs = [];
for (const [key, x] of Object.entries(ma.screens)) {
  const y = mb.screens[key];
  if (!y) continue;
  for (const field of ["horizontal_overflow_px", "axe_violations", "layout_shift"])
    if (x[field] !== y[field]) metricDiffs.push(`${key}: ${field} ${x[field]} → ${y[field]}`);
  if (x.small_text.nodes !== y.small_text.nodes) metricDiffs.push(`${key}: small_text ${x.small_text.nodes} → ${y.small_text.nodes}`);
  if ((x.tap_targets?.under_44 ?? null) !== (y.tap_targets?.under_44 ?? null)) metricDiffs.push(`${key}: tap_targets ${x.tap_targets?.under_44} → ${y.tap_targets?.under_44}`);
  if (x.console_errors.length !== y.console_errors.length) metricDiffs.push(`${key}: console_errors ${x.console_errors.length} → ${y.console_errors.length}`);
}
for (const [name, x] of Object.entries(ma.flows)) {
  const y = mb.flows[name];
  if (!y) continue;
  for (const field of ["steps", "clicks", "fields_typed", "key_presses", "completed"])
    if (x[field] !== y[field]) metricDiffs.push(`flow ${name}: ${field} ${x[field]} → ${y[field]}`);
}

const result = { images_compared: same + differing.length, identical: same, differing, only_in_first: onlyA, only_in_second: onlyB, metric_differences: metricDiffs };
if (process.argv.includes("--json")) console.log(JSON.stringify(result, null, 2));
else {
  console.log(`${same} of ${same + differing.length} paired images identical; ${metricDiffs.length} metric differences.`);
  for (const k of differing) console.log(`  image differs: ${k}`);
  for (const k of onlyA) console.log(`  only in first: ${k}`);
  for (const k of onlyB) console.log(`  only in second: ${k}`);
  for (const d of metricDiffs) console.log(`  metric: ${d}`);
}
process.exitCode = differing.length || onlyA.length || onlyB.length || metricDiffs.length ? 1 : 0;
