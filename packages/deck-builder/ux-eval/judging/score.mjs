#!/usr/bin/env node
// Reads judge outputs and reports scores and, for a pairwise packet, a
// keep/reject verdict.
//
//   node judging/score.mjs --key <packet>.key.json <result.json>...   pairwise: de-blind and decide
//   node judging/score.mjs --absolute <result.json>...                 absolute: mean and spread
//   add --json for machine-readable output
//
// Result files hold one judge's JSON reply each. The judge and sample come
// from the file name, <judge>--<sample>.json (for example opus--1.json,
// sol--3.json), or from "judge" and "sample" fields wrapping the reply as
// { "judge": "...", "sample": 1, "output": { ... } }.
//
// Keep rule (pairwise): keep the candidate only if
//   1. a majority of all judge × sample votes prefer it overall,
//   2. the mean signed overall preference for it (+margin for it, −margin
//      against, 0 for a tie) is greater than the noise: the mean, over
//      judges with two or more samples, of the standard deviation of that
//      judge's own signed overall preferences, and
//   3. no hard metric got worse (see hardMetricRegressions).

import { readFileSync } from "node:fs";
import { basename } from "node:path";

const CATEGORIES = ["visual_consistency", "hierarchy_layout", "colour_contrast", "density_text_economy", "navigation_wayfinding",
  "workflow_efficiency", "feedback_states", "responsive_touch", "accessibility", "overall"];

const args = process.argv.slice(2);
const keyIndex = args.indexOf("--key");
const keyFile = keyIndex >= 0 ? args[keyIndex + 1] : null;
const asJson = args.includes("--json");
const files = args.filter((a, i) => !a.startsWith("--") && !(keyIndex >= 0 && i === keyIndex + 1));
if (!files.length || (!keyFile && !args.includes("--absolute"))) {
  console.error("usage: score.mjs --key <packet>.key.json results... | --absolute results... [--json]");
  process.exit(2);
}

const mean = (xs) => (xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : 0);
const sd = (xs) => (xs.length > 1 ? Math.sqrt(xs.reduce((a, x) => a + (x - mean(xs)) ** 2, 0) / (xs.length - 1)) : 0);
const r2 = (x) => Math.round(x * 100) / 100;

function readResult(file) {
  let raw = readFileSync(file, "utf8").trim();
  // Tolerate a fenced code block around the JSON.
  const fenced = raw.match(/```(?:json)?\s*([\s\S]*?)```/);
  if (fenced) raw = fenced[1];
  const data = JSON.parse(raw);
  const [judge, sample = "1"] = basename(file).replace(/\.json$/, "").split("--");
  return { judge: data.judge ?? judge, sample: String(data.sample ?? sample), output: data.output ?? data, file };
}
const results = files.map(readResult);
const judges = [...new Set(results.map((r) => r.judge))];

// ---------- absolute ----------
if (!keyFile) {
  const report = {};
  for (const c of CATEGORIES) {
    const values = results.map((r) => Number(r.output.scores?.[c])).filter(Number.isFinite);
    report[c] = {
      mean: r2(mean(values)), spread: r2(sd(values)), n: values.length,
      by_judge: Object.fromEntries(judges.map((j) => {
        const v = results.filter((r) => r.judge === j).map((r) => Number(r.output.scores?.[c])).filter(Number.isFinite);
        return [j, { mean: r2(mean(v)), spread: r2(sd(v)), n: v.length }];
      })),
    };
  }
  const findings = results.reduce((n, r) => n + (r.output.findings?.length ?? 0), 0);
  if (asJson) console.log(JSON.stringify({ mode: "absolute", judges, samples: results.length, categories: report, findings }, null, 2));
  else {
    console.log(`Absolute scores from ${results.length} outputs (${judges.join(", ")}); ${findings} findings in total.\n`);
    console.log(`${"category".padEnd(24)} mean  spread  ${judges.map((j) => j.padEnd(14)).join("")}`);
    for (const c of CATEGORIES)
      console.log(`${c.padEnd(24)} ${String(report[c].mean).padEnd(5)} ${String(report[c].spread).padEnd(7)} ${judges.map((j) => `${report[c].by_judge[j].mean}±${report[c].by_judge[j].spread}`.padEnd(14)).join("")}`);
  }
  process.exit(0);
}

// ---------- pairwise ----------
const key = JSON.parse(readFileSync(keyFile, "utf8"));
if (key.mode !== "pairwise") throw new Error("--key must be a pairwise packet's key file");
const forCandidate = (cmp) => {
  if (!cmp || cmp.better === "tie") return 0;
  const m = Math.max(0, Math.min(3, Number(cmp.margin) || 0));
  return cmp.better === key.candidate_is ? m : -m;
};

const categories = {};
for (const c of CATEGORIES) {
  const signed = results.map((r) => forCandidate(r.output.comparisons?.[c]));
  categories[c] = {
    mean: r2(mean(signed)), spread: r2(sd(signed)),
    candidate: signed.filter((s) => s > 0).length, baseline: signed.filter((s) => s < 0).length, tie: signed.filter((s) => s === 0).length,
  };
}
const overall = results.map((r) => ({ judge: r.judge, s: forCandidate(r.output.comparisons?.overall) }));
const perJudge = Object.fromEntries(judges.map((j) => {
  const v = overall.filter((o) => o.judge === j).map((o) => o.s);
  return [j, { mean: r2(mean(v)), spread: r2(sd(v)), samples: v.length }];
}));
const noisy = judges.filter((j) => perJudge[j].samples >= 2);
const noise = noisy.length ? mean(noisy.map((j) => sd(overall.filter((o) => o.judge === j).map((o) => o.s)))) : null;
const votesFor = overall.filter((o) => o.s > 0).length;
const majority = votesFor > overall.length / 2;
const meanPreference = mean(overall.map((o) => o.s));
const aboveNoise = noise !== null && meanPreference > noise;

// ---------- hard metrics ----------
function hardMetricRegressions(baseRun, candRun) {
  const load = (run) => JSON.parse(readFileSync(`${run}/metrics.json`, "utf8"));
  const b = load(baseRun), c = load(candRun);
  const out = [];
  const worse = (label, before, after, tolerance = 0) => { if (after > before + tolerance) out.push(`${label}: ${before} → ${after}`); };
  worse("axe violation nodes (all screens)", b.summary.axe_violation_nodes, c.summary.axe_violation_nodes);
  worse("text nodes under 11 px", b.summary.small_text_nodes, c.summary.small_text_nodes);
  worse("tap targets under 44 px", b.summary.tap_targets_under_44, c.summary.tap_targets_under_44);
  worse("console errors", b.summary.console_errors, c.summary.console_errors);
  // Layout shift during load depends on when responses land relative to frames; identical runs differ by up to ~0.02.
  worse("layout shift (sum over screens)", b.summary.layout_shift_total, c.summary.layout_shift_total, 0.05);
  worse("incomplete captures", b.summary.incomplete_captures.length, c.summary.incomplete_captures.length);
  if (c.summary.flows_completed < b.summary.flows_completed) out.push(`flows completed: ${b.summary.flows_completed} → ${c.summary.flows_completed}`);
  for (const [key, m] of Object.entries(b.screens)) {
    const n = c.screens[key];
    if (!n) { out.push(`${key}: captured before, missing now`); continue; }
    worse(`${key}: axe violation nodes`, m.axe_violations, n.axe_violations);
    worse(`${key}: horizontal overflow px`, m.horizontal_overflow_px, n.horizontal_overflow_px);
    worse(`${key}: console errors`, m.console_errors.length, n.console_errors.length);
  }
  for (const [name, f] of Object.entries(b.flows)) {
    const g = c.flows[name];
    if (!g) { out.push(`flow ${name}: missing`); continue; }
    if (f.completed && !g.completed) out.push(`flow ${name}: no longer completes`);
    if (f.completed && g.completed) {
      worse(`flow ${name}: clicks`, f.clicks, g.clicks);
      worse(`flow ${name}: steps`, f.steps, g.steps);
    }
  }
  // The public page budget: stay inside it, and do not grow by more than 500 bytes.
  const bb = b.public_bundle?.gzip_js_bytes, cb = c.public_bundle?.gzip_js_bytes;
  if (cb == null) out.push("public bundle: not measured");
  else {
    if (cb >= 110000) out.push(`public bundle over budget: ${cb} bytes`);
    if (bb != null) worse("public bundle gzip JS bytes", bb, cb, 500);
  }
  return out;
}
const regressions = hardMetricRegressions(key.baseline, key.candidate);

const reasons = [];
if (!majority) reasons.push(`only ${votesFor} of ${overall.length} votes prefer the candidate overall`);
// One sample per judge (the default, to save usage): noise cannot be
// measured, so require every vote for the candidate and a mean of at least +1.
if (noise === null) {
  if (votesFor !== overall.length || meanPreference < 1)
    reasons.push(`single samples: needs every vote for the candidate and a mean of +1 or more (got ${votesFor}/${overall.length}, ${r2(meanPreference)})`);
}
else if (!aboveNoise) reasons.push(`mean preference ${r2(meanPreference)} is not above the noise ${r2(noise)}`);
if (regressions.length) reasons.push(`${regressions.length} hard metric(s) got worse`);
const verdict = reasons.length ? "reject" : "keep";

const report = {
  mode: "pairwise", verdict, reasons, votes: { total: overall.length, for_candidate: votesFor, majority },
  overall: { mean_preference: r2(meanPreference), noise: noise === null ? null : r2(noise), above_noise: aboveNoise },
  per_judge: perJudge, categories, hard_metric_regressions: regressions,
  packet: { pairs: key.pairs, byte_identical_pairs: key.byte_identical_pairs },
};
if (asJson) console.log(JSON.stringify(report, null, 2));
else {
  console.log(`Verdict: ${verdict.toUpperCase()}${reasons.length ? ` (${reasons.join("; ")})` : ""}\n`);
  console.log(`Overall: ${votesFor}/${overall.length} votes for the candidate; mean preference ${r2(meanPreference)} (−3…+3); noise ${noise === null ? "unknown" : r2(noise)}.`);
  for (const j of judges) console.log(`  ${j}: mean ${perJudge[j].mean}, spread ${perJudge[j].spread}, ${perJudge[j].samples} sample(s)`);
  console.log(`\n${"category".padEnd(24)} mean   spread  for/against/tie`);
  for (const c of CATEGORIES) console.log(`${c.padEnd(24)} ${String(categories[c].mean).padEnd(6)} ${String(categories[c].spread).padEnd(7)} ${categories[c].candidate}/${categories[c].baseline}/${categories[c].tie}`);
  console.log(`\nHard metrics: ${regressions.length ? regressions.map((r) => `\n  worse: ${r}`).join("") : "none worse"}`);
  console.log(`Packet: ${key.byte_identical_pairs} of ${key.pairs} image pairs byte-identical.`);
}
