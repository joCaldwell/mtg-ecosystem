import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, ParsedLine } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Card wordings from the cached Scryfall Oracle corpus. CR 702.122a–b
// (crew), 702.171a/c (saddle), 702.184a/c (station), 609.4.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const thisCreature = { ref: "this", noun: "creature" } as const;
function line(text: string, typeLine = "Creature — Human Pilot"): ParsedLine {
  const result = parseOracleText(text, "Test Card", { typeLine });
  assert.equal(result.lines.length, 1, text);
  return result.lines[0];
}
function ability(text: string, typeLine?: string): Ability {
  const parsed = line(text, typeLine);
  assert.ok(parsed.ok, `${text}: ${parsed.ok ? "" : parsed.error}`);
  return JSON.parse(JSON.stringify(parsed.ability)) as Ability;
}
function fails(text: string, typeLine?: string): void {
  assert.equal(line(text, typeLine).ok, false, text);
}

describe("crew, saddle and station contributions (CR 702.122, 702.171, 702.184)", () => {
  it("adds to the creature's power for the listed payments only", () => {
    // Hotshot Mechanic; Cloudspire Captain.
    assert.deepEqual(ability("This creature crews Vehicles as though its power were 2 greater."), {
      kind: "static", effect: { static: "crew-contribution", what: thisCreature, actions: ["crew"], value: { value: "power-plus", amount: 2 } },
    });
    assert.deepEqual(ability("This creature saddles Mounts and crews Vehicles as though its power were 2 greater."), {
      kind: "static", effect: { static: "crew-contribution", what: thisCreature, actions: ["saddle", "crew"], value: { value: "power-plus", amount: 2 } },
    });
  });

  it("substitutes toughness, a different rule from a power bonus", () => {
    // Giant Ox; Interface Ace; Tapestry Warden.
    assert.deepEqual(ability("This creature crews Vehicles using its toughness rather than its power.", "Creature — Ox"), {
      kind: "static", effect: { static: "crew-contribution", what: thisCreature, actions: ["crew"], value: { value: "toughness" } },
    });
    assert.deepEqual(ability("This creature saddles Mounts and crews Vehicles using its toughness rather than its power."), {
      kind: "static", effect: { static: "crew-contribution", what: thisCreature, actions: ["saddle", "crew"], value: { value: "toughness" } },
    });
    assert.deepEqual(ability("Each creature you control with toughness greater than its power stations permanents using its toughness rather than its power.", "Creature — Spirit"), {
      kind: "static", effect: { static: "crew-contribution", actions: ["station"], value: { value: "toughness" },
        what: { ref: "each", filter: { types: ["creature"], control: "you",
          properties: [{ property: "toughness", comparison: { op: "gt", value: { amount: "attribute", of: { ref: "it" }, attribute: "power" } } }] } } },
    });
  });

  it("is the ability of a created Pilot token", () => {
    // Prodigy's Prototype.
    const created = ability("Whenever one or more Vehicles you control attack, create a 1/1 colorless Pilot creature token with \"This token crews Vehicles as though its power were 2 greater.\"", "Artifact — Vehicle");
    assert.equal(created.kind, "triggered");
    const step = created.kind === "triggered" ? created.effects[0].steps[0] : undefined;
    assert.deepEqual(step?.effect === "create-token" && "abilities" in step.token && step.token.abilities, [{
      gained: "quoted", ability: { kind: "static", effect: { static: "crew-contribution", what: { ref: "this", noun: "token" },
        actions: ["crew"], value: { value: "power-plus", amount: 2 } } },
    }]);
  });

  it("rejects mismatched objects, repeated actions and other counterfactuals", () => {
    fails("This creature crews Mounts as though its power were 2 greater.");
    fails("This creature saddles Vehicles as though its power were 2 greater.");
    fails("This creature crews Vehicles and crews Vehicles as though its power were 2 greater.");
    fails("This creature crews Vehicles as though its power were 2 less.");
    fails("This creature crews Vehicles as though its toughness were 2 greater.");
    fails("This creature crews Vehicles using its power rather than its toughness.");
    fails("This creature crews Vehicles using its toughness rather than their power.");
    fails("This creature crews Vehicles as though its power were 2 greater and can't block.");
    fails("Target creature crews Vehicles as though its power were 2 greater.");
  });
});
