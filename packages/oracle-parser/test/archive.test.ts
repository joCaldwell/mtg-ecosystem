import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { buildArchive as build, publishArchive, verifyArchiveFile } from "../../../scripts/archive-parser.ts";
import { evaluateCorpus as evaluate, sha256 } from "../../../scripts/validate-parser.ts";
import { readCards, type ScryfallCard } from "../../../scripts/lib/scryfall.ts";
import { paperEvidenceFixture } from "./paper-evidence-fixture.ts";

const evidence = paperEvidenceFixture();
const buildArchive = (options: Parameters<typeof build>[0]) => build({ paperEvidence: evidence, ...options });
const evaluateCorpus = (cards: ScryfallCard[]) => evaluate(cards, { paperEvidence: evidence });

const snapshot = { revision: "fixture-revision", sourceSha256: "a".repeat(64) };
const fingerprint = () => snapshot;
function card(id: string, oracle_text = "Flying", extra: Partial<ScryfallCard> = {}): ScryfallCard {
  return { id, oracle_id: id, name: `Fixture ${id}`, layout: "normal", digital: false, games: ["paper"], border_color: "black", oracle_text, ...extra };
}
async function temporary(run: (directory: string, cacheFile: string, outputFile: string) => Promise<void>) {
  const directory = await mkdtemp(path.join(os.tmpdir(), "oracle-archive-test-"));
  try { await run(directory, path.join(directory, "source.json"), path.join(directory, "archive.jsonl")); }
  finally { await rm(directory, { recursive: true, force: true }); }
}

describe("lossless nonexecutable parser archives", () => {
  it("persists target identities and unresolved reference obligations separately from parse acceptance", async () => temporary(async (_dir, cacheFile, outputFile) => {
    const cards = [card("target", "Target creature gets +1/+1 until end of turn.", { type_line: "Instant" }),
      card("unbound", "Draw that many cards.", { type_line: "Sorcery" })];
    await writeFile(cacheFile, JSON.stringify(cards));
    const archive = buildArchive({ cacheFile, fingerprint });
    const report = evaluateCorpus(cards);
    assert.equal(archive.manifest.schemaVersion, 3);
    assert.equal(archive.manifest.cardsOk, 2);
    assert.deepEqual(archive.manifest.bindingSummary, report.bindingSummary);
    for (const record of archive.records) {
      assert.equal(record.status, "parsed");
      if (record.status !== "parsed") assert.fail("Both complete syntactic parses must be stored");
      assert.equal(record.binding.executable, false);
      assert.equal(record.bindingSha256, sha256(JSON.stringify(record.binding)));
      assert.equal(record.bindingSha256, report.outcomes[record.oracleIdentity].bindingSha256);
    }
    const target = archive.records[0], unbound = archive.records[1];
    if (target.status !== "parsed" || unbound.status !== "parsed") assert.fail("Expected parsed records");
    const selectors = target.binding.nodes.filter(node => node.node === "selection" && node.selection === "target");
    assert.equal(selectors.length, 1);
    const selector = selectors[0];
    assert.ok(selector.node === "selection" && "selectionId" in selector.expression && selector.expression.selectionId === "line:0:0");
    assert.equal(unbound.binding.fullyBound, false);
    assert.ok(unbound.binding.counts.unresolved > 0);
    assert.ok(unbound.binding.references.some(ref => ref.binding === "unresolved" && "amount" in ref.expression && ref.expression.amount === "that-much"));
    await publishArchive(archive, outputFile, fingerprint);
    await verifyArchiveFile(outputFile, archive);
    const persisted = (await readFile(outputFile, "utf8")).trimEnd().split("\n").map(line => JSON.parse(line));
    assert.deepEqual(persisted[2].binding, unbound.binding);
  }));
  it("passes card and face metadata through acceptance and archival parsing", async () => temporary(async (_dir, cacheFile) => {
    // Exact cached Oracle text: the parentheses are room graph edges, so a
    // text-only parse of the remaining effects cannot represent this card.
    const dungeon = card("dungeon", "Cave Entrance — Scry 1. (Leads to: Goblin Lair, Mine Tunnels)\nGoblin Lair — Create a 1/1 red Goblin creature token. (Leads to: Storeroom, Dark Pool)\nMine Tunnels — Create a Treasure token. (Leads to: Dark Pool, Fungi Cavern)\nStoreroom — Put a +1/+1 counter on target creature. (Leads to: Temple of Dumathoin)\nDark Pool — Each opponent loses 1 life and you gain 1 life. (Leads to: Temple of Dumathoin)\nFungi Cavern — Target creature gets -4/-0 until your next turn. (Leads to: Temple of Dumathoin)\nTemple of Dumathoin — Draw a card.", { name: "Lost Mine of Phandelver", type_line: "Dungeon" });
    const faceMetadata = card("face-types", "", { card_faces: [{ name: "Room face", oracle_text: "Cave Entrance — Scry 1.", type_line: "Dungeon" }] });
    const attraction = card("attraction", "Visit — Draw a card.", { type_line: "Artifact — Attraction" });
    const cards = [dungeon, faceMetadata, attraction];
    await writeFile(cacheFile, JSON.stringify(cards));
    // The Attraction's visit ability is represented (CR 702.159a),
    // so only the two dungeon fixtures remain unsupported.
    assert.equal(evaluateCorpus(cards).cardsOk, 1);
    const archive = buildArchive({ cacheFile, fingerprint });
    assert.equal(archive.manifest.cardsChecked, 3);
    assert.equal(archive.manifest.cardsOk, 1);
    assert.deepEqual(archive.manifest.excluded, {});
    assert.deepEqual(archive.records.map(record => record.status), ["unsupported", "unsupported", "parsed"]);
    const visited = archive.records[2];
    if (visited.status === "parsed" && visited.faces[0].status === "attempted" && visited.faces[0].result.ok) {
      assert.deepEqual(JSON.parse(JSON.stringify(visited.faces[0].result.abilities)), [{
        kind: "visit", effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: { amount: "fixed", value: 1 } }] }],
      }]);
    } else assert.fail("The Attraction's visit ability must be archived as parsed");
    const first = archive.records[0];
    if (first.status === "unsupported" && first.faces[0].status === "attempted") {
      assert.ok(first.faces[0].result.lines.every(line => !line.ok && /dungeon room structure/.test(line.error)));
    } else assert.fail("Dungeon must retain an explicit unsupported parse result");
    assert.deepEqual(archive.records[0].source, dungeon);
  }));

  it("validates optional type lines without requiring them on every card", () => {
    for (const type_line of [undefined, null, "Dungeon"])
      assert.equal(readCards(JSON.stringify([card("type", "", { type_line })])).length, 1);
    assert.throws(() => readCards(JSON.stringify([{ ...card("invalid"), type_line: 42 }])), /Invalid Scryfall/);
    assert.throws(() => readCards(JSON.stringify([card("invalid-face", "", { card_faces: [{ name: "Face", oracle_text: "Flying", type_line: 42 as unknown as string }] })])), /Invalid Scryfall/);
  });

  it("retains every complete source record, all faces, failures, and unchanged scoreboard counts", async () => temporary(async (_dir, cacheFile, outputFile) => {
    const fullSource = {
      ...card("one"), mana_cost: "{X}{G/P}", power: "*", toughness: "1+*", defense: "7",
      legalities: { commander: "legal" }, colors: ["G"], future_field: { nested: [null, true, { text: "é\nunchanged" }] },
    };
    const cards = [
      fullSource,
      card("unsupported", "Unsupported mechanic."),
      card("null", "", { oracle_text: null }),
      card("faces", "", { layout: "transform", card_faces: [{ name: "Front", oracle_text: "Vigilance" }, { name: "Back" }] }),
      card("digital", "Unsupported digital instruction", { digital: true }),
      card("duplicate", "Flying", { oracle_id: "one" }),
      card("empty", ""),
      card("reversible", "", { oracle_id: null, layout: "reversible_card", card_faces: [
        { name: "Face A", oracle_id: "face-a", oracle_text: "Flying" },
        { name: "Face B", oracle_id: "face-b", oracle_text: "Haste" },
      ] }),
    ];
    const input = JSON.stringify(cards);
    await writeFile(cacheFile, input);
    const archive = buildArchive({ cacheFile, fingerprint });
    const report = evaluateCorpus(cards);
    for (const key of ["scope", "paperEvidence", "inputRecords", "excluded", "cardsChecked", "cardsOk", "linesChecked", "linesOk", "bindingSummary"] as const) {
      assert.deepEqual(archive.manifest[key], report[key], key);
    }
    assert.equal(archive.manifest.artifactKind, "oracle-ast-archive");
    assert.equal(archive.manifest.executable, false);
    assert.equal(archive.manifest.corpus.sha256, sha256(input));
    assert.deepEqual(archive.records.map(record => record.source), cards);
    assert.deepEqual(archive.records.map(record => record.status), ["parsed", "unsupported", "unsupported", "unsupported", "excluded", "excluded", "parsed", "parsed"]);
    assert.equal(archive.records[7].oracleIdentity, "face-a/face-b");
    for (const record of archive.records) {
      assert.equal(record.sourceSha256, sha256(JSON.stringify(cards[record.sourceIndex])));
      if (record.status === "parsed") assert.equal(record.astSha256, report.outcomes[record.oracleIdentity].astSha256);
      else assert.equal("astSha256" in record, false);
      if (record.status !== "excluded") assert.equal(record.bindingSha256, report.outcomes[record.oracleIdentity].bindingSha256);
    }
    const nullText = archive.records[2];
    assert.equal(nullText.status, "unsupported");
    if (nullText.status === "unsupported") assert.deepEqual(nullText.faces, [{ sourceFaceIndex: 0, status: "missing-oracle-text", name: "Fixture null" }]);
    await publishArchive(archive, outputFile, fingerprint);
    await verifyArchiveFile(outputFile, archive);
    const persisted = (await readFile(outputFile, "utf8")).trimEnd().split("\n").map(line => JSON.parse(line));
    assert.deepEqual(persisted.slice(1).map(record => record.source), cards);
    assert.equal(persisted[0].executable, false);
    assert.deepEqual(persisted[1].faces[0].result.abilities, [{ kind: "keywords", keywords: [{ keyword: "flying" }] }]);
    assert.equal(await readFile(path.join(path.dirname(outputFile), archive.manifest.corpus.snapshotFile), "utf8"), input);
  }));

  it("produces deterministic bytes and verifies full ASTs after deserialization", async () => temporary(async (_dir, cacheFile, outputFile) => {
    await writeFile(cacheFile, JSON.stringify([card("one", "Ward—Pay 3 life.\n{T}: Draw a card.")]));
    const first = buildArchive({ cacheFile, fingerprint });
    await publishArchive(first, outputFile, fingerprint);
    const bytes = await readFile(outputFile);
    const second = buildArchive({ cacheFile, fingerprint });
    assert.deepEqual(first, second);
    await publishArchive(second, outputFile, fingerprint);
    assert.deepEqual(await readFile(outputFile), bytes);
  }));

  it("rejects truncation and changed serialized records", async () => temporary(async (_dir, cacheFile, outputFile) => {
    await writeFile(cacheFile, JSON.stringify([card("one"), card("two")]));
    const archive = buildArchive({ cacheFile, fingerprint });
    await publishArchive(archive, outputFile, fingerprint);
    const full = await readFile(outputFile, "utf8");
    await writeFile(outputFile, full.split("\n").slice(0, 2).join("\n") + "\n");
    await assert.rejects(verifyArchiveFile(outputFile, archive), /Truncated/);
    await writeFile(outputFile, full.replace('"flying"', '"haste"'));
    await assert.rejects(verifyArchiveFile(outputFile, archive), /round-trip mismatch/);
  }));

  it("does not replace an existing artifact when the source snapshot changes", async () => temporary(async (dir, cacheFile, outputFile) => {
    await writeFile(cacheFile, JSON.stringify([card("one")]));
    await writeFile(outputFile, "previous archive");
    const archive = buildArchive({ cacheFile, fingerprint });
    let calls = 0;
    await assert.rejects(publishArchive(archive, outputFile, () => ++calls === 1 ? snapshot : { ...snapshot, sourceSha256: "b".repeat(64) }), /sources changed/);
    assert.equal(await readFile(outputFile, "utf8"), "previous archive");
    assert.deepEqual((await readdir(dir)).sort(), ["archive.jsonl", archive.manifest.corpus.snapshotFile, archive.manifest.paperEvidence.snapshotFile, "source.json"].sort());
  }));

  it("rejects source changes during parsing before an artifact is published", async () => temporary(async (_dir, cacheFile) => {
    await writeFile(cacheFile, JSON.stringify([card("one")]));
    let calls = 0;
    assert.throws(() => buildArchive({ cacheFile, fingerprint: () => ++calls === 1 ? snapshot : { ...snapshot, revision: "changed" } }), /sources changed/);
  }));

  it("independently checks accepted AST and source hashes before replacing files", async () => temporary(async (dir, cacheFile, outputFile) => {
    await writeFile(cacheFile, JSON.stringify([card("one")]));
    await writeFile(outputFile, "previous archive");
    const archive = buildArchive({ cacheFile, fingerprint });
    const record = archive.records[0];
    assert.equal(record.status, "parsed");
    if (record.status !== "parsed") throw new Error("fixture should parse");
    record.astSha256 = "0".repeat(64);
    await assert.rejects(publishArchive(archive, outputFile, fingerprint), /AST hash mismatch/);
    assert.equal(await readFile(outputFile, "utf8"), "previous archive");
    const second = buildArchive({ cacheFile, fingerprint });
    second.records[0].sourceSha256 = "0".repeat(64);
    await assert.rejects(publishArchive(second, outputFile, fingerprint), /source hash mismatch/);
    assert.equal(await readFile(outputFile, "utf8"), "previous archive");
    assert.deepEqual((await readdir(dir)).sort(), ["archive.jsonl", archive.manifest.corpus.snapshotFile, archive.manifest.paperEvidence.snapshotFile, "source.json"].sort());
  }));

  it("preserves exact source bytes including number spelling and rejects a corrupt existing snapshot", async () => temporary(async (dir, cacheFile, outputFile) => {
    const input = JSON.stringify([{ ...card("one"), future_number: "PRECISE" }], null, 2).replace('"PRECISE"', "9007199254740993") + "\n";
    await writeFile(cacheFile, input);
    const archive = buildArchive({ cacheFile, fingerprint });
    await publishArchive(archive, outputFile, fingerprint);
    const previous = await readFile(outputFile);
    const snapshotFile = path.join(dir, archive.manifest.corpus.snapshotFile);
    assert.deepEqual(await readFile(snapshotFile), Buffer.from(input));
    await writeFile(snapshotFile, "corruption");
    await assert.rejects(publishArchive(archive, outputFile, fingerprint), /snapshot hash mismatch/);
    assert.deepEqual(await readFile(outputFile), previous);
    assert.equal(await readFile(snapshotFile, "utf8"), "corruption");
    assert.ok((await readdir(dir)).every(file => !file.endsWith(".tmp")));
  }));

  it("rejects non-finite source values that JSON serialization would silently erase", async () => temporary(async (_dir, cacheFile) => {
    const input = JSON.stringify([{ ...card("one"), future_number: "OVERFLOW" }]).replace('"OVERFLOW"', "1e400");
    await writeFile(cacheFile, input);
    assert.throws(() => buildArchive({ cacheFile, fingerprint }), /Non-finite/);
  }));
});
