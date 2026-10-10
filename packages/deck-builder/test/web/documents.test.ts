import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { editDiff, editTitle, isTextEdit } from "../../web/src/domain/documents.ts";
import { documentEdit } from "./fixtures.ts";

describe("isTextEdit", () => {
  test("profile, thesis and constraints are text; engines are not", () => {
    for (const kind of ["profile", "thesis", "constraints"] as const)
      assert.equal(isTextEdit(documentEdit({ kind })), true, kind);
    for (const kind of ["engine_set", "engine_remove"] as const)
      assert.equal(isTextEdit(documentEdit({ kind })), false, kind);
  });
});

describe("editDiff", () => {
  test("diffs the proposed text against the base it was written against", () => {
    const edit = documentEdit({
      kind: "thesis",
      current: "something newer",
      payload: { base: "one\ntwo", content: "one\nthree" },
    });
    assert.deepEqual(editDiff(edit).map((l) => l.kind), ["same", "del", "add"]);
  });

  test("with no base, falls back to the current text", () => {
    const edit = documentEdit({ kind: "profile", current: "old", payload: { content: "new" } });
    assert.deepEqual(editDiff(edit), [
      { kind: "del", text: "old" },
      { kind: "add", text: "new" },
    ]);
  });

  test("a first document diffs against nothing, and clearing one deletes it", () => {
    assert.deepEqual(editDiff(documentEdit({ payload: { base: "", content: "hello" } })), [
      { kind: "add", text: "hello" },
    ]);
    assert.deepEqual(editDiff(documentEdit({ payload: { base: "bye" } })), [{ kind: "del", text: "bye" }]);
  });

  test("an engine edit has no diff", () => {
    assert.deepEqual(editDiff(documentEdit({ kind: "engine_set", payload: { engine_name: "Test Engine" } })), []);
  });
});

describe("editTitle", () => {
  test("names what the edit touches", () => {
    assert.equal(editTitle(documentEdit({ kind: "profile" })), "Edit to your profile");
    assert.equal(editTitle(documentEdit({ kind: "thesis" })), "Edit to the thesis");
    assert.equal(editTitle(documentEdit({ kind: "constraints" })), "Edit to the constraints");
    assert.equal(
      editTitle(documentEdit({ kind: "engine_set", payload: { engine_name: "Test Engine" } })),
      "Add or change the engine “Test Engine”",
    );
    assert.equal(
      editTitle(documentEdit({ kind: "engine_remove", payload: { engine_name: "Test Engine" } })),
      "Remove the engine “Test Engine”",
    );
  });
});
