// packages/oracle-parser/tests/parser.test.ts
import { describe, it, expect } from "vitest";
import { parseOracleText } from "../src/index.js";

describe("Oracle Text Parser", () => {
  it("should parse keyword abilities", () => {
    const tree = parseOracleText("Flying");
    expect(tree).toBeDefined();
    expect(tree.text).toBe("Flying<EOF>");
  });

  it("should parse activated abilities", () => {
    const tree = parseOracleText("{T}: Draw a card.");
    expect(tree).toBeDefined();
    expect(tree.text).toBe("{T}:Drawacard.<EOF>");
  });

  it("should parse triggered abilities", () => {
    const tree = parseOracleText("Whenever target creature dies, draw a card.");
    expect(tree).toBeDefined();
    expect(tree.text).toBe("Whenevertargetcreaturedies,drawacard.<EOF>");
  });
});
