import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, readFile, readdir, rm, utimes, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { gzipSync } from "node:zlib";
import { ingest } from "../../../scripts/ingest-scryfall.ts";
import { evaluateCorpus as evaluate, sha256, buildReport as build } from "../../../scripts/validate-parser.ts";
import { readCards, readOracleManifest, type ScryfallCard } from "../../../scripts/lib/scryfall.ts";
import { paperEvidenceFixture } from "./paper-evidence-fixture.ts";

const evidence = paperEvidenceFixture();
const evaluateCorpus = (cards: ScryfallCard[]) => evaluate(cards, { paperEvidence: evidence });
const buildReport = (cacheFile: string) => build(cacheFile, evidence);

function card(id: string, text = "Flying", extra: Partial<ScryfallCard> = {}): ScryfallCard {
  return { id, oracle_id: id, name: `Fixture ${id}`, layout: "normal", digital: false, games: ["paper"], border_color: "black", oracle_text: text, ...extra };
}
async function directory(run: (dir: string, cacheFile: string) => Promise<void>) {
  const dir = await mkdtemp(path.join(os.tmpdir(), "oracle-ingest-test-"));
  try { await run(dir, path.join(dir, "oracle-cards.json")); }
  finally { await rm(dir, { recursive: true, force: true }); }
}
function download(response: () => Response): typeof fetch {
  let requests = 0;
  return async () => {
    if (requests++ === 0) return Response.json({ data: [{
      type: "oracle_cards", download_uri: "https://example.invalid/cards.json", updated_at: "2026-08-26T00:00:00Z",
    }] });
    return response();
  };
}
const quiet = () => {};
function jsonlDownload(bytes: Buffer): typeof fetch {
  let request = 0;
  return async (url, options) => {
    assert.ok(options?.headers);
    if (request++ === 0) return Response.json({ data: [{ type: "oracle_cards",
      jsonl_download_uri: "https://example.invalid/oracle-cards.jsonl.gz", updated_at: "2026-10-10T21:01:55Z", compressed_size: bytes.length }] });
    assert.equal(url, "https://example.invalid/oracle-cards.jsonl.gz");
    let offset = 0;
    return new Response(new ReadableStream<Uint8Array>({ pull(controller) {
      if (offset === bytes.length) { controller.close(); return; }
      controller.enqueue(bytes.subarray(offset, offset += Math.min(17, bytes.length - offset)));
    } }));
  };
}

describe("atomic corpus ingestion", () => {
  it("consumes the current gzip JSONL manifest while preserving original record number and string spellings", async () => directory(async (dir, cacheFile) => {
    const first = JSON.stringify({ ...card("one", "Flying", { name: "Fixture élève" }), future_number: "EXACT" }).replace('"EXACT"', "9007199254740993");
    const second = JSON.stringify(card("two", "Vigilance"));
    const data = `${first}\r\n${second}\n`;
    await writeFile(cacheFile, JSON.stringify([card("old")])); await utimes(cacheFile, 0, 0);
    assert.equal(await ingest({ cacheFile, fetcher: jsonlDownload(gzipSync(data)), log: quiet }), "downloaded");
    const stored = await readFile(cacheFile, "utf8");
    assert.equal(stored, `[\n${first}\r,\n${second}\n]\n`);
    assert.deepEqual(readCards(stored).map(c => c.id), ["one", "two"]);
    assert.deepEqual(await readdir(dir), ["oracle-cards.json"]);
  }));
  it("preserves the previous cache for truncated gzip, invalid UTF-8, malformed JSONL and empty downloads", async () => {
    const good = gzipSync(JSON.stringify(card("one")) + "\n");
    const badInputs = [good.subarray(0, good.length - 7), gzipSync(Buffer.from([0xff, 0x0a])), gzipSync(""),
      gzipSync("{}\n"), gzipSync(`${JSON.stringify(card("one"))}\nnot-json\n`),
      gzipSync(`${JSON.stringify(card("one"))},${JSON.stringify(card("two"))}\n`)];
    for (const bytes of badInputs) await directory(async (dir, cacheFile) => {
      const previous = JSON.stringify([card("previous")]);
      await writeFile(cacheFile, previous); await utimes(cacheFile, 0, 0);
      await assert.rejects(ingest({ cacheFile, fetcher: jsonlDownload(bytes), log: quiet }));
      assert.equal(await readFile(cacheFile, "utf8"), previous);
      assert.deepEqual(await readdir(dir), ["oracle-cards.json"]);
    });
  });
  it("selects an explicit bulk format and rejects insecure or unknown compressed formats", () => {
    const entry = { type: "oracle_cards", updated_at: "2026-10-10T21:01:55Z" };
    assert.deepEqual(readOracleManifest({ data: [{ ...entry, jsonl_download_uri: "https://example.invalid/cards.jsonl.gz" }] }),
      { download_uri: "https://example.invalid/cards.jsonl.gz", updated_at: entry.updated_at, format: "gzip-jsonl" });
    assert.deepEqual(readOracleManifest({ data: [{ ...entry, download_uri: "https://example.invalid/cards.json" }] }),
      { download_uri: "https://example.invalid/cards.json", updated_at: entry.updated_at, format: "json-array" });
    for (const uri of ["http://example.invalid/cards.jsonl.gz", "https://example.invalid/cards.zip", "https://example.invalid/cards.jsonl"])
      assert.throws(() => readOracleManifest({ data: [{ ...entry, jsonl_download_uri: uri }] }));
  });
  it("publishes a valid complete replacement", async () => directory(async (dir, cacheFile) => {
    const old = JSON.stringify([card("old")]), replacement = JSON.stringify([card("new")]);
    await writeFile(cacheFile, old); await utimes(cacheFile, 0, 0);
    assert.equal(await ingest({ cacheFile, fetcher: download(() => new Response(replacement)), log: quiet }), "downloaded");
    assert.equal(await readFile(cacheFile, "utf8"), replacement);
    assert.deepEqual(await readdir(dir), ["oracle-cards.json"]);
  }));
  it("rejects invalid UTF-8 in a legacy array and keeps the previous exact cache bytes", async () => directory(async (dir, cacheFile) => {
    const old = Buffer.from(JSON.stringify([card("old")]));
    const invalid = Buffer.from(JSON.stringify([card("new", "Flying", { name: "Invalid marker" })]));
    invalid[invalid.indexOf("marker")] = 0xff;
    await writeFile(cacheFile, old); await utimes(cacheFile, 0, 0);
    await assert.rejects(ingest({ cacheFile, fetcher: download(() => new Response(invalid)), log: quiet }), /Invalid UTF-8/);
    assert.deepEqual(await readFile(cacheFile), old);
    assert.deepEqual(await readdir(dir), ["oracle-cards.json"]);
  }));
  it("replaces a fresh cache with invalid UTF-8 instead of accepting replacement characters", async () => directory(async (_dir, cacheFile) => {
    const invalid = Buffer.from(JSON.stringify([card("cached", "Flying", { name: "Invalid marker" })]));
    invalid[invalid.indexOf("marker")] = 0xff;
    await writeFile(cacheFile, invalid);
    const replacement = JSON.stringify([card("recovered")]);
    assert.equal(await ingest({ cacheFile, fetcher: download(() => new Response(replacement)), log: quiet }), "downloaded");
    assert.equal(await readFile(cacheFile, "utf8"), replacement);
  }));
  it("validates a fresh cache before skipping the network", async () => directory(async (_dir, cacheFile) => {
    await writeFile(cacheFile, JSON.stringify([card("cached")]));
    assert.equal(await ingest({ cacheFile, fetcher: async () => { throw new Error("Unexpected network request"); }, log: quiet }), "cached");
    await writeFile(cacheFile, "[");
    assert.equal(await ingest({ cacheFile, fetcher: download(() => Response.json([card("recovered")])), log: quiet }), "downloaded");
    assert.equal(readCards(await readFile(cacheFile, "utf8"))[0].id, "recovered");
  }));
  it("retains the old corpus after an interrupted stream", async () => directory(async (dir, cacheFile) => {
    const old = JSON.stringify([card("old")]);
    await writeFile(cacheFile, old); await utimes(cacheFile, 0, 0);
    const fetcher = download(() => {
      let sent = false;
      return new Response(new ReadableStream<Uint8Array>({
        pull(controller) {
          if (!sent) { sent = true; controller.enqueue(new TextEncoder().encode('[{"id":')); }
          else controller.error(new Error("connection lost"));
        },
      }));
    });
    await assert.rejects(ingest({ cacheFile, fetcher, log: quiet }), /connection lost/);
    assert.equal(await readFile(cacheFile, "utf8"), old);
    assert.deepEqual(await readdir(dir), ["oracle-cards.json"]);
  }));
  it("retains the old corpus after invalid JSON, empty data, invalid records, or HTTP failure", async () => {
    for (const response of [() => new Response("["), () => Response.json([]), () => Response.json([{}]), () => new Response("failure", { status: 503 })]) {
      await directory(async (dir, cacheFile) => {
        const old = JSON.stringify([card("old")]);
        await writeFile(cacheFile, old); await utimes(cacheFile, 0, 0);
        await assert.rejects(ingest({ cacheFile, fetcher: download(response), log: quiet }));
        assert.equal(await readFile(cacheFile, "utf8"), old);
        assert.deepEqual(await readdir(dir), ["oracle-cards.json"]);
      });
    }
  });
  it("rejects invalid manifests before publishing anything", async () => directory(async (dir, cacheFile) => {
    await assert.rejects(ingest({ cacheFile, fetcher: async () => Response.json({ data: [] }), log: quiet }), /manifest/);
    assert.deepEqual(await readdir(dir), []);
  }));
});

describe("reproducible acceptance reporting", () => {
  it("separates exclusions and deduplicates oracle identities, not names", () => {
    const report = evaluateCorpus([
      card("one", "Flying", { name: "Shared name" }),
      card("two", "Flying", { name: "Shared name" }),
      card("reprint", "Flying", { oracle_id: "one" }),
      card("digital", "Flying", { digital: true }),
      card("silver", "Flying", { border_color: "silver" }),
      card("acorn", "Flying", { security_stamp: "acorn" }),
      card("token", "Flying", { layout: "token" }),
    ]);
    assert.equal(report.cardsChecked, 2); assert.equal(report.cardsOk, 2);
    assert.deepEqual(report.excluded, { duplicate: 1, "no-eligible-paper-printing": 1, "silver-or-acorn": 2, "layout:token": 1 });
  });
  it("includes empty-text cards but fails a missing face's text", () => {
    const report = evaluateCorpus([
      card("vanilla", ""),
      card("null-text", "", { oracle_text: null, card_faces: null }),
      card("faces", "", { layout: "transform", card_faces: [{ name: "front", oracle_text: "Flying" }, { name: "back" }] }),
    ]);
    assert.equal(report.cardsChecked, 3); assert.equal(report.cardsOk, 1);
    assert.equal(report.linesChecked, 1); assert.equal(report.linesOk, 1);
    assert.equal(report.outcomes.faces.ok, false);
    assert.ok(report.failures.some(group => group.key === "missing-oracle-text"));
  });
  it("accepts nullable API fields without treating them as empty rules text", () => {
    const cards = readCards(JSON.stringify([card("nullable", "", { oracle_text: null, card_faces: null })]));
    assert.equal(cards[0].oracle_text, null);
    assert.equal(evaluateCorpus(cards).cardsOk, 0);
  });
  it("records exact input and source fingerprints and stable AST hashes", async () => directory(async (_dir, cacheFile) => {
    const corpus = JSON.stringify([card("one")]); await writeFile(cacheFile, corpus);
    const a = buildReport(cacheFile), b = buildReport(cacheFile);
    assert.deepEqual(a, b);
    assert.equal(a.corpus.sha256, sha256(corpus));
    assert.match(a.parser.sourceSha256, /^[a-f0-9]{64}$/);
    assert.match(a.outcomes.one.astSha256!, /^[a-f0-9]{64}$/);
    const changed = evaluateCorpus([card("one", "Vigilance")]);
    assert.notEqual(changed.outcomes.one.astSha256, a.outcomes.one.astSha256);
  }));
  it("groups at the actual failure and retains full examples", () => {
    const report = evaluateCorpus([card("a", "{T}: draw a banana."), card("b", "+1: draw a banana.")]);
    assert.equal(report.failures.length, 1);
    assert.equal(report.failures[0].key, 'at "banana"');
    assert.equal(report.failures[0].count, 2);
    assert.deepEqual(report.failures[0].examples.map(e => e.text), ["{T}: draw a banana.", "+1: draw a banana."]);
  });
});
