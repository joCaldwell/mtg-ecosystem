import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import {
  buildCardNameCatalog, mapToOriginal, normalizeOracleText, normalizeOracleTextTracked, projectNormalizedRange, Utf8Index,
  type ByteRange, type CardNameSource, type Edit, type MappedNormalization, type TrackedNormalization,
} from "../src/index.ts";
import { replayNormalizationEdits } from "../src/printed/replay.ts";
import { rewriteSourcePronouns, sourcePronounRewrites } from "../src/source-pronouns.ts";
import { normalizeOracleText as legacyNormalize } from "./support/legacy-normalize.ts";

interface Vector {
  id: string;
  text: string;
  name: string | null;
  typeLine: string | null;
  catalog?: CardNameSource[];
  normalized: string;
  edits: Edit[] | null;
  reason?: Extract<TrackedNormalization, { status: "unmappable" }>["reason"];
}

const vectors: Vector[] = JSON.parse(readFileSync(new URL(
  "../../game-engine/tests/fixtures/printed-text-golden.json", import.meta.url,
), "utf8"));

function mapped(...args: Parameters<typeof normalizeOracleTextTracked>): MappedNormalization {
  const trace = normalizeOracleTextTracked(...args);
  assert.equal(trace.status, "mapped", JSON.stringify(trace));
  return trace;
}

function fixture(id: string) {
  const vector = vectors.find(item => item.id === id);
  assert.ok(vector);
  return vector;
}

describe("printed-text golden vectors", () => {
  for (const vector of vectors) {
    it(vector.id, () => {
      const { text, normalized, edits, reason } = vector;
      const name = vector.name ?? undefined, typeLine = vector.typeLine ?? undefined;
      const catalog = vector.catalog && buildCardNameCatalog(vector.catalog);
      const args = [text, name, catalog, typeLine] as const;
      const ordinary = normalizeOracleText(...args);
      const trace = normalizeOracleTextTracked(...args);
      assert.deepEqual(ordinary, legacyNormalize(...args));
      assert.deepEqual(trace.lines, ordinary);
      assert.equal(ordinary.join("\n"), normalized);
      if (edits === null) {
        assert.deepEqual(trace, { status: "unmappable", lines: ordinary, reason });
      } else {
        assert.equal(trace.status, "mapped");
        assert.deepEqual(trace.edits, edits);
        assert.equal(replayNormalizationEdits(text, trace, name, catalog, typeLine), normalized);
      }
    });
  }
});

describe("UTF-8 coordinates", () => {
  it("converts complete scalars and ranges", () => {
    const index = new Utf8Index("A😀B");
    assert.deepEqual([0, 1, 3, 4].map(n => index.at(n)), [0, 1, 5, 6]);
    assert.deepEqual(index.range([1, 3]), [1, 5]);
    assert.equal(index.byteLength, Buffer.byteLength("A😀B"));
    const mixed = new Utf8Index("Aé−😀B");
    assert.deepEqual([0, 1, 2, 3, 5, 6].map(n => mixed.at(n)), [0, 1, 3, 6, 10, 11]);
    assert.equal(new Utf8Index("").at(0), 0);
  });

  it("rejects invalid UTF-16 and endpoints without changing ordinary normalization", () => {
    const index = new Utf8Index("A😀B");
    for (const endpoint of [-1, 1.5, 2, 5, Infinity, NaN, 2 ** 32]) {
      assert.throws(() => index.at(endpoint), RangeError);
    }
    assert.throws(() => index.range([0, 2]), /Split surrogate pair/);
    assert.throws(() => index.range([3, 1]), /Reversed UTF-16 range/);
    for (const text of ["A\ud800B", "\ud800", "\udc00B", "\ud800\ud800", "A(\ud800)B"]) {
      assert.throws(() => new Utf8Index(text), /Lone surrogate/);
      const lines = legacyNormalize(text);
      assert.deepEqual(normalizeOracleText(text), lines);
      assert.deepEqual(normalizeOracleTextTracked(text), { status: "unmappable", lines, reason: "invalid-unicode" });
    }
  });
});

describe("byte projection", () => {
  it("maps Boseiju and the later copied tokens with exact byte deltas", () => {
    const { text, name, typeLine } = fixture("boseiju");
    const trace = mapped(text, name!, undefined, typeLine!);
    assert.deepEqual(trace.lineRanges, [[0, 16], [17, 127]]);
    assert.deepEqual(mapToOriginal(trace, [0, 1]), [0, 7]);
    assert.deepEqual(mapToOriginal(trace, [1, 2]), [7, 8]);
    assert.deepEqual(mapToOriginal(trace, [46, 55]), [52, 61]);
    assert.deepEqual(mapToOriginal(trace, [97, 107]), [103, 113]);
    const possessive = mapped("Urza's power", "Urza, Lord High Artificer");
    assert.deepEqual(mapToOriginal(possessive, [0, 3]), [0, 6]);
  });

  it("projects the two words in a name contraction to the same original", () => {
    const { text, name, typeLine } = fixture("name-pronouns");
    const trace = mapped(text, name!, undefined, typeLine!);
    for (const n of [[55, 56], [57, 59], [55, 59]] satisfies ByteRange[]) {
      assert.deepEqual(projectNormalizedRange(trace, n), { original: [[61, 65]], anchors: [] });
    }
  });

  it("anchors generated enclosure quotes and preserves quoted copies", () => {
    const catalog = buildCardNameCatalog([{ name: "Squadron Hawk", layout: "normal" }]);
    const trace = mapped("named Squadron Hawk", undefined, catalog);
    assert.deepEqual(projectNormalizedRange(trace, [6, 7]), { original: [], anchors: [6] });
    assert.deepEqual(projectNormalizedRange(trace, [20, 21]), { original: [], anchors: [19] });
    assert.deepEqual(projectNormalizedRange(trace, [6, 21]), { original: [[6, 19]], anchors: [6, 19] });
    assert.deepEqual(mapToOriginal(trace, [6, 7]), [6, 19]);
    const quoted = mapped('named "Squadron Hawk"', undefined, catalog);
    assert.deepEqual(projectNormalizedRange(quoted, [6, 7]), { original: [[6, 21]], anchors: [] });
  });

  it("maps across astral text, original-only reminders, and an end anchor", () => {
    const trace = mapped("A😀 (🦀 nested) B");
    assert.deepEqual(trace.lineRanges, [[0, 7]]);
    assert.deepEqual(mapToOriginal(trace, [1, 5]), [1, 5]);
    assert.deepEqual(mapToOriginal(trace, [6, 7]), [20, 21]);
    assert.deepEqual(mapToOriginal(trace, [1, 7]), [1, 21]);
    assert.throws(() => mapToOriginal(trace, [2, 5]), /Invalid normalized UTF-8 range/);
    assert.throws(() => mapToOriginal(trace, [6, 5]), /Invalid normalized UTF-8 range/);
    assert.deepEqual(projectNormalizedRange(trace, [7, 7]), { original: [], anchors: [21] });
    assert.deepEqual(mapToOriginal(mapped("(reminder)"), [0, 0]), [0, 10]);
    assert.deepEqual(mapToOriginal(mapped(""), [0, 0]), [0, 0]);
    const { text, name, typeLine } = fixture("station-reminder");
    const station = mapped(text, name!, undefined, typeLine!);
    assert.deepEqual(station.edits, [{ edit: "gap", o: [90, 246], n: [90, 91], reminder: [[91, 245]] }]);
    assert.match(Buffer.from(text).subarray(91, 245).toString(), /artifact creature at 9\+\.\)$/);
    assert.deepEqual(mapToOriginal(station, [91, 93]), [246, 248]);
  });
});

describe("normalization admissibility", () => {
  it("rejects split scalars, bad lengths, nonmonotone axes, and corrupted copy bytes", () => {
    const text = "😀 − −";
    const trace = mapped(text);
    for (const [mutate, error] of [
      [(t: MappedNormalization) => { t.edits[0].o[0] = 2; }, /Invalid edit ranges/],
      [(t: MappedNormalization) => { t.edits[0].n[0] = 2; }, /Invalid edit ranges/],
      [(t: MappedNormalization) => { t.edits.reverse(); }, /(?:Unchanged runs differ|nonmonotone)/],
      [(t: MappedNormalization) => { t.edits[1].o[0] = t.edits[0].o[0]; }, /nonmonotone/],
      [(t: MappedNormalization) => { t.originalBytes--; }, /lengths disagree/],
      [(t: MappedNormalization) => { t.lineRanges[0][0]++; }, /Invalid normalized physical line ranges/],
      [(t: MappedNormalization) => { t.lines[0] = "🦀 - -"; }, /Unchanged runs differ/],
      [(t: MappedNormalization) => { t.lines[0] = "😀 + -"; }, /Invalid punct replacement/],
    ] as const) {
      const bad = structuredClone(trace);
      mutate(bad);
      assert.throws(() => replayNormalizationEdits(text, bad), error);
    }
  });

  it("rejects out-of-order, unbalanced or split-scalar reminder ranges and a non-whitespace gap", () => {
    const text = "A (🦀) (nested (pair)) B";
    const trace = mapped(text);
    const gap = trace.edits[0] as Extract<Edit, { edit: "gap" }>;
    for (const reminder of [[], [[3, 8], gap.reminder[1]], [[3, 4], gap.reminder[1]], [...gap.reminder].reverse()] as ByteRange[][]) {
      const bad = { ...trace, edits: [{ ...gap, reminder }] };
      assert.throws(() => replayNormalizationEdits(text, bad), /Invalid reminder range|Non-whitespace gap/);
    }
  });

  it("rejects rewrites with the wrong face form, pronoun role, or literal proof even when the bytes replay", () => {
    const face = mapped("Kara attacks.", "Kara, Test Hero");
    assert.throws(() => replayNormalizationEdits("Kara attacks.", {
      ...face, edits: [{ ...face.edits[0], edit: "self-name", form: "full" }],
    }, "Kara, Test Hero"), /Inadmissible self-name/);
    const typeLine = "Legendary Creature — Human";
    const pronouns = mapped("Kara attacks, he wins.", "Kara", undefined, typeLine);
    assert.throws(() => replayNormalizationEdits("Kara attacks, he wins.", {
      ...pronouns, edits: [pronouns.edits[0], { ...pronouns.edits[1], edit: "pronoun", role: "object", as: "neuter" }],
    }, "Kara", undefined, typeLine), /Inadmissible pronoun/);
    const catalog = buildCardNameCatalog([{ name: "Hawk", layout: "normal" }]);
    const literal = mapped("named Hawk", undefined, catalog);
    assert.throws(() => replayNormalizationEdits("named Hawk", literal), /Inadmissible literal-name/);
    assert.throws(() => replayNormalizationEdits("named Hawk", {
      ...literal, edits: [{ ...literal.edits[0], edit: "literal-name", slot: "create", via: "catalog" }],
    }, undefined, catalog), /Inadmissible literal-name/);
    const quoted = 'named "Hawk"';
    assert.throws(() => replayNormalizationEdits(quoted, { ...mapped(quoted, undefined, catalog), edits: [] },
      undefined, catalog), /Missing literal-name edit/);
  });

  it("rejects admissible spellings used in the wrong normalization context", () => {
    const text = "Whenever Kara attacks, he loses.";
    const typeLine = "Legendary Creature — Human";
    const name = "Kara";
    const valid = mapped(text, name, undefined, typeLine);
    const fake = structuredClone(valid);
    fake.lines[0] = fake.lines[0].replace("it loses", "~ loses");
    fake.normalizedBytes--;
    fake.lineRanges[0][1]--;
    const edit = fake.edits[1];
    assert.equal(edit.edit, "pronoun");
    edit.as = "name";
    edit.n[1]--;
    assert.throws(() => replayNormalizationEdits(text, fake, name, undefined, typeLine),
      /Replayed text differs from legacy normalization/);
  });

  it("rolls back staged pronoun rewrites when a later pronoun fails", () => {
    const good = "Whenever ~ attacks, he wins.";
    const bad = "Whenever ~ attacks, he taps her friend.";
    assert.deepEqual(sourcePronounRewrites(good), [{ start: 20, end: 22, replacement: "it", role: "subject", as: "neuter" }]);
    assert.deepEqual(sourcePronounRewrites(bad), []);
    assert.equal(rewriteSourcePronouns(bad), bad);
  });
});
