import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { buildPaperEvidence } from "../../../scripts/build-paper-evidence.ts";
import { paperExclusion, readPaperEvidence, PAPER_SCOPE } from "../../../scripts/lib/paper-evidence.ts";
import { buildReport, evaluateCorpus, sha256 } from "../../../scripts/validate-parser.ts";
import { buildArchive, publishArchive, verifyArchiveFile } from "../../../scripts/archive-parser.ts";
import type { ScryfallCard } from "../../../scripts/lib/scryfall.ts";
import { paperEvidenceFixture } from "./paper-evidence-fixture.ts";

function card(id: string, extra: Partial<ScryfallCard> = {}): ScryfallCard {
  return { id, oracle_id: id, name: "Shared name", layout: "normal", digital: false,
    games: ["paper"], border_color: "black", oracle_text: "Flying", ...extra };
}
describe("paper identity scope, independent of the selected Oracle printing", () => {
  it("includes digital representatives only with matching eligible paper identity evidence", () => {
    const evidence = paperEvidenceFixture([card("paper-print", { oracle_id: "paper-oracle" })]);
    const digital = card("online-print", { digital: true, games: ["mtgo"], oracle_id: "paper-oracle" });
    const onlineOnly = card("online-only", { digital: true, games: ["arena"] });
    const cards = [digital, onlineOnly, card("paper-other"), card("silver", { border_color: "silver" }),
      card("acorn", { security_stamp: "acorn" }), card("token", { layout: "token", oracle_id: "paper-oracle" })];
    const report = evaluateCorpus(cards, { paperEvidence: evidence });
    assert.equal(report.scope, PAPER_SCOPE);
    assert.equal(report.cardsChecked, 2);
    assert.equal(report.cardsOk, 2);
    assert.deepEqual(Object.keys(report.outcomes).sort(), ["paper-oracle", "paper-other"]);
    assert.deepEqual(report.excluded, { "no-eligible-paper-printing": 1, "silver-or-acorn": 2, "layout:token": 1 });
    assert.equal(paperExclusion(card("unknown-games", { games: undefined }), evidence), "no-eligible-paper-printing");
    assert.equal(paperExclusion(card("empty-games", { games: [] }), evidence), "no-eligible-paper-printing");
  });
  it("does not use a silver representative to hide an eligible reprint; preserves reversible face identity", () => {
    const reversible = card("paper-reversible", { oracle_id: null, layout: "reversible_card", card_faces: [
      { name: "A", oracle_id: "a" }, { name: "B", oracle_id: "b" }] });
    const evidence = paperEvidenceFixture([card("black", { oracle_id: "reprint" }), reversible]);
    assert.equal(paperExclusion(card("silver", { oracle_id: "reprint", border_color: "silver" }), evidence), undefined);
    assert.equal(paperExclusion({ ...reversible, digital: true, games: ["mtgo"] }, evidence), undefined);
    assert.equal(paperExclusion(card("same-face-name", { digital: true, games: ["mtgo"], oracle_id: "a" }), evidence), "no-eligible-paper-printing");
  });
  it("rejects digital, acorn, silver, missing-games, duplicate, and malformed evidence", () => {
    for (const extra of [{ digital: true }, { security_stamp: "acorn" }, { border_color: "silver" }, { games: [] }, { games: undefined }])
      assert.throws(() => paperEvidenceFixture([card("bad", extra)]), /ineligible/);
    assert.throws(() => paperEvidenceFixture([card("one"), card("two", { oracle_id: "one" })]), /Duplicate/);
    assert.throws(() => readPaperEvidence(Buffer.from("{}")), /Invalid/);
  });
  it("builds deterministic identity evidence from actual complete JSONL printing records", async () => {
    const dir = await mkdtemp(path.join(os.tmpdir(), "paper-evidence-"));
    try {
      const file = path.join(dir, "printings.jsonl");
      const cards = [card("z-print", { oracle_id: "same" }), card("a-print", { oracle_id: "same" }),
        card("online", { digital: true, games: ["arena"] }), card("silver", { border_color: "silver" })];
      const source = cards.map(c => JSON.stringify(c)).join("\n") + "\n";
      await writeFile(file, source);
      const evidence = await buildPaperEvidence(file, { downloadUri: "https://example.invalid/printings.jsonl.gz", updatedAt: "2026-10-10T00:00:00Z" });
      assert.equal(evidence.document.source.sha256, sha256(source));
      assert.equal(evidence.document.source.bytes, Buffer.byteLength(source));
      assert.equal(evidence.document.source.records, 4);
      assert.equal(evidence.byIdentity.size, 1);
      assert.equal(evidence.byIdentity.get("same")?.id, "a-print");
      assert.equal("oracle_text" in evidence.document.printings[0], false);
    } finally { await rm(dir, { recursive: true, force: true }); }
  });
  it("rejects invalid UTF-8 and multiple records on one JSONL line without altered card facts", async () => {
    const dir = await mkdtemp(path.join(os.tmpdir(), "paper-invalid-source-"));
    try {
      const file = path.join(dir, "source.json"), printing = JSON.stringify(card("fixture"));
      const invalid = Buffer.from(printing.replace("Shared name", "CORRUPT"));
      invalid[invalid.indexOf("CORRUPT")] = 0xff;
      await writeFile(file, Buffer.concat([Buffer.from("["), invalid, Buffer.from("]")]));
      const paperEvidence = paperEvidenceFixture();
      assert.throws(() => buildReport(file, paperEvidence), /encoded data/);
      assert.throws(() => buildArchive({ cacheFile: file, paperEvidence }), /encoded data/);
      const invalidEvidence = Buffer.from(paperEvidenceFixture([card("fixture")]).bytes);
      invalidEvidence[invalidEvidence.indexOf("Shared name")] = 0xff;
      assert.throws(() => readPaperEvidence(invalidEvidence), /encoded data/);
      const provenance = { downloadUri: "https://example.invalid/printings.jsonl.gz", updatedAt: "2026-10-10T00:00:00Z" };
      await writeFile(file, invalid);
      await assert.rejects(buildPaperEvidence(file, provenance), /encoded data/);
      await writeFile(file, `${printing},${printing}\n`);
      await assert.rejects(buildPaperEvidence(file, provenance), /exactly one/);
    } finally { await rm(dir, { recursive: true, force: true }); }
  });
  it("stores scope evidence with the full digital Oracle source and detects evidence corruption", async () => {
    const dir = await mkdtemp(path.join(os.tmpdir(), "paper-scope-archive-"));
    try {
      const paperEvidence = paperEvidenceFixture([card("paper-print", { oracle_id: "same" })]);
      const digital = card("digital-print", { oracle_id: "same", digital: true, games: ["mtgo"] });
      const cacheFile = path.join(dir, "source.json"), outputFile = path.join(dir, "archive.jsonl");
      await writeFile(cacheFile, JSON.stringify([digital]));
      const fingerprint = () => ({ revision: "fixture", sourceSha256: "a".repeat(64) });
      const archive = buildArchive({ cacheFile, fingerprint, paperEvidence });
      assert.equal(archive.manifest.cardsChecked, 1);
      assert.equal(archive.records[0].status, "parsed");
      assert.deepEqual(archive.records[0].source, digital);
      for (const snapshot of [archive.manifest.corpus.snapshotFile, archive.manifest.paperEvidence.snapshotFile])
        await assert.rejects(publishArchive(archive, path.join(dir, snapshot), fingerprint), /must not overwrite/);
      await publishArchive(archive, outputFile, fingerprint);
      await verifyArchiveFile(outputFile, archive);
      const evidenceFile = path.join(dir, archive.manifest.paperEvidence.snapshotFile);
      assert.deepEqual(await readFile(evidenceFile), paperEvidence.bytes);
      const previous = await readFile(outputFile);
      await writeFile(evidenceFile, "corrupt evidence");
      await assert.rejects(verifyArchiveFile(outputFile, archive), /snapshot hash mismatch/);
      await assert.rejects(publishArchive(archive, outputFile, fingerprint), /snapshot hash mismatch/);
      assert.deepEqual(await readFile(outputFile), previous);
    } finally { await rm(dir, { recursive: true, force: true }); }
  });
});
