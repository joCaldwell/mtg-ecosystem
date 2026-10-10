import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { diffLines } from "../../web/src/domain/diff.ts";

describe("diffLines", () => {
  test("two empty documents have no lines", () => {
    assert.deepEqual(diffLines("", ""), []);
  });

  test("from empty, every line is an add; to empty, every line is a deletion", () => {
    assert.deepEqual(diffLines("", "one\ntwo"), [
      { kind: "add", text: "one" },
      { kind: "add", text: "two" },
    ]);
    assert.deepEqual(diffLines("one\ntwo", ""), [
      { kind: "del", text: "one" },
      { kind: "del", text: "two" },
    ]);
  });

  test("identical text is all same", () => {
    assert.deepEqual(diffLines("a\nb", "a\nb"), [
      { kind: "same", text: "a" },
      { kind: "same", text: "b" },
    ]);
  });

  test("a changed line is a deletion and an add around the unchanged ones", () => {
    const out = diffLines("a\nb\nc", "a\nB\nc");
    assert.deepEqual(out.map((l) => l.kind), ["same", "del", "add", "same"]);
    assert.deepEqual(out.filter((l) => l.kind !== "same").map((l) => l.text), ["b", "B"]);
  });

  test("an inserted line is just an add", () => {
    assert.deepEqual(diffLines("a\nc", "a\nb\nc"), [
      { kind: "same", text: "a" },
      { kind: "add", text: "b" },
      { kind: "same", text: "c" },
    ]);
  });

  test("a blank line inside a document is a line", () => {
    assert.equal(diffLines("a\n\nb", "a\n\nb").length, 3);
  });
});
