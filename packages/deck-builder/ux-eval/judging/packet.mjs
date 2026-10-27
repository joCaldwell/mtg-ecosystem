#!/usr/bin/env node
// Builds a judge packet: the folder a judge is given, and nothing else.
//
//   node judging/packet.mjs absolute <run>                      [options]
//   node judging/packet.mjs pairwise <baseline-run> <candidate-run> [options]
//
// Options:
//   --out <dir>       where packets go (default <UX_EVAL_ROOT>/packets)
//   --only <regex>    keep only captures whose key matches (flow steps: "<flow>/<NN>")
//   --no-flows        leave out the flows
//   --full            include full-page shots as well as first-viewport shots
//
// The packet folder holds brief.md, rubric.md, prompt.md, index.md and the
// images under neutral names (the capture keys). It never contains a run id,
// date, commit or anything about the loop. For a pairwise packet, which run
// is "Version 1" is chosen at random per packet, and the mapping is written
// next to the folder as <packet>.key.json, which the judges never get.

import { UX_EVAL_ROOT } from "../lib/paths.mjs";
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createHash, randomBytes, randomInt } from "node:crypto";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const opt = (name) => { const i = args.indexOf(`--${name}`); return i >= 0 ? args[i + 1] : undefined; };
const has = (name) => args.includes(`--${name}`);
const positional = args.filter((a, i) => !a.startsWith("--") && !(i > 0 && ["--out", "--only"].includes(args[i - 1])));
const [mode, ...runs] = positional;
if (!["absolute", "pairwise"].includes(mode) || runs.length !== (mode === "absolute" ? 1 : 2)) {
  console.error("usage: packet.mjs absolute <run> | pairwise <baseline-run> <candidate-run> [--out dir] [--only regex] [--no-flows] [--full]");
  process.exit(2);
}
const ONLY = opt("only") ? new RegExp(opt("only")) : null;
const OUT = resolve(opt("out") ?? join(UX_EVAL_ROOT, "packets"));
const id = randomBytes(5).toString("hex");
const packet = join(OUT, id);
mkdirSync(packet, { recursive: true });

/** Every capture of a run as { key, file, viewport, state, caption, group }. */
function captures(run) {
  const m = JSON.parse(readFileSync(join(run, "manifest.json"), "utf8"));
  const out = [];
  for (const s of m.screens) {
    const base = { group: s.screen, viewport: s.viewport, state: s.state, caption: s.caption, signed_in: s.signed_in };
    out.push({ ...base, key: s.key, file: s.files.viewport, view: "first viewport" });
    if (has("full") && s.files.full) out.push({ ...base, key: `${s.key}--full`, file: s.files.full, view: "full page" });
  }
  if (!has("no-flows"))
    for (const f of m.flows)
      f.steps.forEach((step, i) => out.push({
        group: `flow: ${f.key}`, flow: f.key, flowCaption: f.caption, key: step.key, file: step.file,
        viewport: f.viewport, state: `step ${i + 1} of ${f.steps.length}`, caption: step.caption,
      }));
  return out.filter((c) => !ONLY || ONLY.test(c.key));
}
const imageName = (key) => `${key.replace(/\//g, "--")}.png`;
const vpText = (v) => `${v.width} × ${v.height}${v.touch ? ", touch" : ""}`;
const sha = (f) => createHash("sha256").update(readFileSync(f)).digest("hex");

for (const [from, to] of [["brief.md", "brief.md"], ["rubric.md", "rubric.md"], [mode === "absolute" ? "judge-absolute.md" : "judge-pairwise.md", "prompt.md"]])
  copyFileSync(join(HERE, from), join(packet, to));

const lines = [];
let key;
if (mode === "absolute") {
  const run = resolve(runs[0]);
  const list = captures(run);
  mkdirSync(join(packet, "images"), { recursive: true });
  lines.push("# Index", "", "Every image in `images/`. Read `brief.md`, then `rubric.md`, then `prompt.md`.", "");
  let group = null;
  for (const c of list) {
    if (c.group !== group) {
      group = c.group;
      lines.push("", `## ${group}${c.flowCaption ? `: ${c.flowCaption}` : ""}`, "", "| Key | Image | Viewport | State | Caption |", "| --- | --- | --- | --- | --- |");
    }
    copyFileSync(join(run, c.file), join(packet, "images", imageName(c.key)));
    lines.push(`| \`${c.key}\` | images/${imageName(c.key)} | ${vpText(c.viewport)} | ${c.state}${c.view === "full page" ? " (full page)" : ""} | ${c.caption} |`);
  }
  key = { packet: id, mode, run, images: list.length };
} else {
  const baseline = resolve(runs[0]), candidate = resolve(runs[1]);
  const swap = randomInt(2) === 1;
  const v1 = swap ? candidate : baseline, v2 = swap ? baseline : candidate;
  const A = captures(v1), B = captures(v2);
  const byKey = (list) => new Map(list.map((c) => [c.key, c]));
  const mA = byKey(A), mB = byKey(B);
  // Screens pair by key. Flows pair by flow: either version may have more steps.
  const order = [...A, ...B.filter((c) => !mA.has(c.key))];
  for (const d of ["v1", "v2"]) mkdirSync(join(packet, d), { recursive: true });
  lines.push("# Index", "", "Each key is shown as Version 1 (`v1/`) and Version 2 (`v2/`). Read `brief.md`, then `rubric.md`, then `prompt.md`.", "");
  let group = null, identical = 0, pairs = 0;
  for (const c of order) {
    if (c.group !== group) {
      group = c.group;
      lines.push("", `## ${group}${c.flowCaption ? `: ${c.flowCaption}` : ""}`, "", "| Key | Version 1 | Version 2 | Viewport | State | Caption (Version 1 / Version 2) |", "| --- | --- | --- | --- | --- | --- |");
    }
    const a = mA.get(c.key), b = mB.get(c.key);
    if (a) copyFileSync(join(v1, a.file), join(packet, "v1", imageName(c.key)));
    if (b) copyFileSync(join(v2, b.file), join(packet, "v2", imageName(c.key)));
    if (a && b) { pairs++; if (sha(join(v1, a.file)) === sha(join(v2, b.file))) identical++; }
    const caption = a && b && a.caption === b.caption ? a.caption : `${a?.caption ?? "(no such step)"} / ${b?.caption ?? "(no such step)"}`;
    const state = a && b && a.state !== b.state ? `${a.state} / ${b.state}` : (a ?? b).state;
    lines.push(`| \`${c.key}\` | ${a ? `v1/${imageName(c.key)}` : "absent"} | ${b ? `v2/${imageName(c.key)}` : "absent"} | ${vpText(c.viewport)} | ${state}${c.view === "full page" ? " (full page)" : ""} | ${caption} |`);
  }
  key = { packet: id, mode, baseline, candidate, version_1: v1, version_2: v2, candidate_is: swap ? "version_1" : "version_2", pairs, byte_identical_pairs: identical, only_in_version_1: A.filter((c) => !mB.has(c.key)).map((c) => c.key), only_in_version_2: B.filter((c) => !mA.has(c.key)).map((c) => c.key) };
}
writeFileSync(join(packet, "index.md"), lines.join("\n").replace(/^\n+/, "") + "\n");
const keyFile = join(OUT, `${id}.key.json`);
writeFileSync(keyFile, JSON.stringify({ private: "Never give this file to a judge.", ...key }, null, 2) + "\n");

// The folder must not name a run, date or commit: refuse to finish if it does.
const forbidden = [...runs.map((r) => basename(resolve(r))), /\b[0-9a-f]{40}\b/, /\b20\d\d-\d\d-\d\dT/];
const text = ["index.md", "brief.md", "rubric.md", "prompt.md"].map((f) => readFileSync(join(packet, f), "utf8")).join("\n");
for (const f of forbidden) if (typeof f === "string" ? text.includes(f) : f.test(text)) throw new Error(`Packet text reveals ${f}`);
if (!existsSync(join(packet, "index.md"))) throw new Error("no index");
console.log(JSON.stringify({ packet, key: keyFile, ...(mode === "pairwise" ? { pairs: key.pairs, byte_identical_pairs: key.byte_identical_pairs } : { images: key.images }) }));
