import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { LexError, lex } from "../src/lexer.ts";

describe("lex", () => {
  it("lexes words, numbers, symbols, punctuation", () => {
    assert.deepEqual(lex("{T}: Add {G}."), [
      { kind: "symbol", value: "T" },
      { kind: "punct", value: ":" },
      { kind: "word", value: "add", raw: "Add", possessive: false },
      { kind: "symbol", value: "G" },
      { kind: "punct", value: "." },
    ]);
  });

  it("keeps hybrid and Phyrexian symbols as single tokens", () => {
    assert.deepEqual(lex("{G/P}{2/W}"), [
      { kind: "symbol", value: "G/P" },
      { kind: "symbol", value: "2/W" },
    ]);
  });

  it("flags possessives and keeps contractions whole", () => {
    const tokens = lex("its owner's hand can't");
    assert.deepEqual(tokens, [
      { kind: "word", value: "its", raw: "its", possessive: false },
      { kind: "word", value: "owner", raw: "owner", possessive: true },
      { kind: "word", value: "hand", raw: "hand", possessive: false },
      { kind: "word", value: "can't", raw: "can't", possessive: false },
    ]);
  });

  it("handles plural possessives (owners')", () => {
    assert.deepEqual(lex("their owners' hands"), [
      { kind: "word", value: "their", raw: "their", possessive: false },
      { kind: "word", value: "owners", raw: "owners", possessive: true },
      { kind: "word", value: "hands", raw: "hands", possessive: false },
    ]);
  });

  it("lexes self-references and possessive self-references", () => {
    assert.deepEqual(lex("~'s power"), [
      { kind: "selfref", possessive: true },
      { kind: "word", value: "power", raw: "power", possessive: false },
    ]);
  });

  it("throws loudly on unknown characters instead of dropping them", () => {
    // The old ANTLR pipeline silently swallowed these and reported success.
    assert.throws(() => lex("Flying ; % ##"), LexError);
  });
});
