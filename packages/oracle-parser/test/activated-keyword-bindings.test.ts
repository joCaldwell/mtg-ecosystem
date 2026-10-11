import assert from "node:assert/strict";
import { it } from "node:test";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";

// CR 702.142 (boast) and 702.177 (exhaust) add intrinsic conditions and
// activation-history obligations even when the printed body has simple refs.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
it("does not claim intrinsic activated keyword history is already bound", () => {
  for (const keyword of ["Boast", "Exhaust"]) {
    const result = parseOracleText(`${keyword} — {T}: Draw a card.`, "Keyword Test", { typeLine: "Creature", includeSourceProvenance: true });
    assert.ok(result.ok);
    const binding = analyzeCardBindings({ identity: `keyword-${keyword}`, source: { name: "Keyword Test", layout: "normal", type_line: "Creature" }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(binding.fullyBound, false, keyword);
    assert.ok(binding.diagnostics.some(d => d.code === "unsupported-scope" && d.path.endsWith("/keyword")), keyword);
  }
});
