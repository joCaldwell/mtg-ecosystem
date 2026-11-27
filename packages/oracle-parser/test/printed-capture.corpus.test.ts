import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { writeFile } from "node:fs/promises";
import { test } from "node:test";
import { parseOracleText, parseOracleTextWithPrintedEvidence } from "../src/index.ts";
import type { PrintedBlock, PrintedFace, TokenRun } from "../src/printed/schema.ts";
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

const familyNames = ["modify-characteristics", "gain-abilities", "get-counters", "sacrifice", "pump", "move-zone",
  "exile", "destroy", "search", "damage", "return-to-hand", "discard", "draw", "mill", "create-token"] as const;
const witnessGroups = [
  ["Boseiju, Who Shelters All"],
  ["Delighted Halfling"],
  ["Junk Diver", "Terastodon"],
  ["Secret Rendezvous", "Wedgelight Rammer"],
] as const;

function valuesAtPaths(value: unknown, path = "", values = new Map<string, Record<string, unknown>>()) {
  if (Array.isArray(value)) value.forEach((item, index) => valuesAtPaths(item, `${path}/${index}`, values));
  else if (value && typeof value === "object") {
    const object = value as Record<string, unknown>;
    values.set(path, object);
    for (const [key, child] of Object.entries(object))
      valuesAtPaths(child, path + "/" + key.replaceAll("~", "~0").replaceAll("/", "~1"), values);
  }
  return values;
}

function auditBlock(face: Extract<PrintedFace, { status: "read" }>,
  block: Extract<PrintedBlock, { status: "read" }>, ability: unknown, source: Buffer) {
  const values = valuesAtPaths(ability);
  return block.nodes.map(node => {
    const value = values.get(node.path);
    assert.ok(value, node.path);
    const printed = node.basis === "recorded" ? [...node.span, ...(node.folds ?? [])].map(([line, start, end]) => {
      const row = face.lines[line];
      assert.equal(row.status, "lexed");
      return source.toString("utf8", row.tokens[start].o[0], row.tokens[end - 1].o[1]);
    }).join(" ") : "";
    return { node, value, printed };
  });
}

test("printed evidence preserves frozen R5 results, bindings and resolution", { skip: !archivePath && "set MTG_R5_ARCHIVE" }, async t => {
  assert.ok(archivePath);
  const { catalog, observedArchiveSha256 } = await frozenR5(archivePath);
  const counts = { attempted: 0, legacyDiffs: 0, captureDiffs: 0, bindingDiffs: 0, resolutionDiffs: 0,
    failedBlocks: 0, failedLexLines: 0, unmappable: 0, targetIntroductions: 0, targetMentions: 0, targetMentionsInReadBlocks: 0 };
  const bases = { recorded: 0, hull: 0, implicit: 0, alias: 0, unlocated: 0 };
  const candidates = { nesting: 0, overlap: 0, conflict: 0, unlocatedSelections: 0, mutationGuardDemotions: 0 };
  const wrongSpans = { verbRecordedYou: 0, regenerationForbidden: 0, unpreventable: 0, optional: 0 };
  const families = familyNames.map(family => ({ family, roots: 0, unlocatedRoots: 0, nodes: 0, unlocated: 0 }));
  const witnesses = witnessGroups.map(names => ({ names, observed: new Set<string>(), failedBlocks: 0, unlocated: [] as string[] }));
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
        const sourceText = Buffer.from(source.oracle_text!);
        const witness = witnesses.find(({ names }) => names.some(name => name === source.name));
        if (witness) witness.observed.add(source.name);
        for (const [index, block] of read.printed.blocks.entries()) {
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
          if (block.status === "failed") {
            counts.failedBlocks++;
            if (witness) witness.failedBlocks++;
            continue;
          }
          const parsedLine = read.result.lines[index];
          assert.ok(parsedLine?.ok, site);
          const roots = new Map<string, typeof families[number]>();
          for (const { node, value, printed } of auditBlock(read.printed, block, parsedLine.ability, sourceText)) {
            bases[node.basis]++;
            if (node.basis === "unlocated") witness?.unlocated.push(`${site} block ${index}${node.path}`);
            const family = families.find(item => item.family === value.effect);
            if (family) {
              roots.set(node.path, family);
              family.roots++;
              if (node.basis === "unlocated") family.unlocatedRoots++;
            }
            let parent = node.path;
            while (parent && !roots.has(parent)) parent = parent.slice(0, parent.lastIndexOf("/"));
            const owner = roots.get(parent);
            if (owner) {
              owner.nodes++;
              if (node.basis === "unlocated") owner.unlocated++;
            }
            if (node.basis !== "recorded") continue;
            if (value.player === "you" && !/\byou(?:r(?:self|s)?)?\b/iu.test(printed)) wrongSpans.verbRecordedYou++;
            if (value.regenerationForbidden === true && !/\bregenerated\b/iu.test(printed)) wrongSpans.regenerationForbidden++;
            if (value.effect === "damage" && value.unpreventable === true && !/\bprevented\b/iu.test(printed)) wrongSpans.unpreventable++;
            if (value.optional === true && !/\bmay\b/iu.test(printed)) wrongSpans.optional++;
          }
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
  const receipt = { archiveSha256: observedArchiveSha256, legacyResultSha256: digest.digest("hex"),
    ...counts, nodeCount, bases, percentages, candidates, emitted, wrongSpans,
    families: families.map(family => ({ ...family, unlocatedPercent: +(family.unlocated * 100 / family.nodes).toFixed(4),
      unlocatedRootPercent: +(family.unlocatedRoots * 100 / family.roots).toFixed(4) })),
    witnesses: witnesses.map(({ names, observed, failedBlocks, unlocated }) =>
      ({ names, observed: [...observed], failedBlocks, unlocatedCount: unlocated.length, unlocated })),
  };
  t.diagnostic(JSON.stringify(receipt));
  if (process.env.MTG_R5_CAPTURE_RECEIPT)
    await writeFile(process.env.MTG_R5_CAPTURE_RECEIPT, JSON.stringify(receipt, null, 2) + "\n");
  assert.equal(counts.attempted, 34_464);
  assert.equal(counts.unmappable, 0);
  assert.deepEqual(failures, []);
  assert.deepEqual(emitted, { nesting: 0, overlap: 0, conflict: 0 });
  assert.deepEqual(wrongSpans, { verbRecordedYou: 0, regenerationForbidden: 0, unpreventable: 0, optional: 0 });
  assert.equal(candidates.mutationGuardDemotions, 0);
  assert.equal(candidates.unlocatedSelections, 0);
  for (const { names, observed, failedBlocks, unlocated } of witnesses) {
    assert.deepEqual([...observed].sort(), [...names].sort());
    assert.equal(failedBlocks, 0);
    assert.deepEqual(unlocated, []);
  }
});
