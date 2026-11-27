import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { test } from "node:test";
import { parseOracleText, parseOracleTextWithPrintedEvidence } from "../src/index.ts";
import type { PrintedBlock, TokenRun } from "../src/printed/schema.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";
import { resolveCard } from "../src/semantic/resolution.ts";
import { frozenR5, readRows } from "./support/frozen-r5.ts";

const archivePath = process.env.MTG_R5_ARCHIVE;

function defectCount(block: Extract<PrintedBlock, { status: "read" }>) {
  const byPath = new Map(block.nodes.map(node => [node.path, node]));
  const regions = new Map<string, TokenRun[]>();
  let nesting = 0, overlap = 0, conflict = 0;
  for (const node of block.nodes) {
    if (node.basis === "alias") {
      const original = byPath.get(node.of);
      if (!original || original.basis === "alias" || original.basis === "unlocated") conflict++;
      continue;
    }
    if (node.basis !== "recorded" && node.basis !== "hull") continue;
    const coverage = node.basis === "recorded" ? [...node.span, ...(node.folds ?? [])] : node.span;
    const tokens = coverage.flatMap(([line, start, end]) =>
      Array.from({ length: end - start }, (_, i) => line + ":" + (start + i)));
    const own = new Set(tokens);
    if (tokens.length !== own.size || coverage.some(([line]) => line < block.lines[0] || line >= block.lines[1])) conflict++;
    let parent = node.path;
    while (parent && !regions.has(parent)) parent = parent.slice(0, parent.lastIndexOf("/"));
    if (node.path && regions.has(parent)) {
      const bounds = regions.get(parent)!;
      if (coverage.some(([line, start, end]) => {
        for (let token = start; token < end; token++) {
          if (!bounds.some(([l, from, to]) => l === line && from <= token && token < to)) return true;
        }
        return false;
      })) nesting++;
    }
    for (const [sibling, spans] of regions) {
      if (!sibling || node.path.startsWith(sibling + "/")) continue;
      if (coverage.some(([line, start, end]) => spans.some(([l, from, to]) => l === line && from < end && start < to))) overlap++;
    }
    regions.set(node.path, coverage);
  }
  return { nesting, overlap, conflict };
}

test("printed evidence preserves frozen R5 results, bindings and resolution", { skip: !archivePath && "set MTG_R5_ARCHIVE" }, async t => {
  assert.ok(archivePath);
  const { catalog, observedArchiveSha256 } = await frozenR5(archivePath);
  const counts = { attempted: 0, legacyDiffs: 0, captureDiffs: 0, bindingDiffs: 0, resolutionDiffs: 0,
    failedBlocks: 0, failedLexLines: 0, unmappable: 0, targetIntroductions: 0, targetMentions: 0, targetMentionsInReadBlocks: 0 };
  const bases = { recorded: 0, hull: 0, implicit: 0, alias: 0, unlocated: 0 };
  const candidates = { nesting: 0, overlap: 0, conflict: 0, unlocatedSelections: 0 };
  const emitted = { nesting: 0, overlap: 0, conflict: 0 };
  const failures: string[] = [];
  const digest = createHash("sha256");
  const check = (pass: boolean, key: "legacyDiffs" | "captureDiffs" | "bindingDiffs" | "resolutionDiffs", site: string) => {
    if (pass) return;
    counts[key]++;
    if (failures.length < 12) failures.push(`${key} at ${site}`);
  };

  for await (const row of readRows(archivePath)) {
    if (row.status === "excluded") continue;
    const sources = row.source.card_faces ?? [row.source];
    const faces = row.faces.map(face => {
      const { sourceFaceIndex } = face;
      if (face.status !== "attempted") return { sourceFaceIndex };
      const source = sources[sourceFaceIndex];
      assert.equal(typeof source.oracle_text, "string");
      const context = { cardNames: catalog, layout: row.source.layout, typeLine: source.type_line ?? row.source.type_line ?? undefined };
      const legacy = parseOracleText(source.oracle_text!, source.name, { ...context, includeSourceProvenance: true });
      const read = parseOracleTextWithPrintedEvidence(source.oracle_text!, source.name, context, sourceFaceIndex);
      const actual = JSON.stringify(read.result);
      const expected = JSON.stringify(face.result);
      const site = `${row.sourceIndex}/${sourceFaceIndex} ${source.name}`;
      counts.attempted++;
      check(JSON.stringify(legacy) === expected, "legacyDiffs", site);
      check(actual === expected, "captureDiffs", site);
      digest.update(JSON.stringify([row.sourceIndex, sourceFaceIndex, read.result]) + "\n");
      for (const key of Object.keys(candidates) as (keyof typeof candidates)[]) candidates[key] += read.defects[key];
      if (read.printed.status === "unmappable") counts.unmappable++;
      else {
        counts.failedLexLines += read.printed.lines.filter(line => line.status === "failed").length;
        for (const block of read.printed.blocks) {
          const indices = new Map<string, number>();
          for (const fact of block.facts) {
            if (fact.fact === "target") {
              counts.targetIntroductions++;
              const next = indices.get(fact.scope) ?? 0;
              assert.equal(fact.targetIndex, next, site);
              indices.set(fact.scope, next + 1);
            } else if (fact.fact === "mention" && fact.form === "target") {
              counts.targetMentions++;
              if (block.status === "read") counts.targetMentionsInReadBlocks++;
            }
          }
          if (block.status === "failed") { counts.failedBlocks++; continue; }
          for (const { basis } of block.nodes) bases[basis]++;
          const defects = defectCount(block);
          for (const key of Object.keys(emitted) as (keyof typeof emitted)[]) emitted[key] += defects[key];
        }
      }
      return { sourceFaceIndex, result: read.result };
    });
    const input = { identity: row.oracleIdentity, source: row.source, faces };
    const binding = analyzeCardBindings(input);
    const resolution = resolveCard(input, binding);
    check(JSON.stringify(binding) === JSON.stringify(row.binding), "bindingDiffs", row.source.name);
    check(JSON.stringify(resolution) === JSON.stringify(row.resolution), "resolutionDiffs", row.source.name);
  }

  const nodeCount = Object.values(bases).reduce((sum, count) => sum + count, 0);
  const percentages = Object.fromEntries(Object.entries(bases).map(([basis, count]) => [basis, +(count * 100 / nodeCount).toFixed(4)]));
  t.diagnostic(JSON.stringify({ archiveSha256: observedArchiveSha256, legacyResultSha256: digest.digest("hex"),
    ...counts, nodeCount, bases, percentages, candidates, emitted }));
  assert.equal(counts.attempted, 34_464);
  assert.equal(counts.unmappable, 0);
  assert.deepEqual(failures, []);
  assert.deepEqual(emitted, { nesting: 0, overlap: 0, conflict: 0 });
});
