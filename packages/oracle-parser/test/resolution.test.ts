import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";
import { resolveCard, type BlockResolution, type RefEntry } from "../src/semantic/resolution.ts";

/** Parse, bind and resolve one single-face card; return its blocks. */
function blocks(text: string, typeLine: string, manaCost = "{1}", name = "Fixture Card", layout = "normal"): BlockResolution[] {
  const result = parseOracleText(text, name, { typeLine, layout, includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result.lines));
  const input = { identity: "fixture", source: { name, layout, type_line: typeLine, mana_cost: manaCost }, faces: [{ sourceFaceIndex: 0, result }] };
  const resolution = resolveCard(input, analyzeCardBindings(input));
  return resolution.faces[0].blocks!;
}
const refs = (block: BlockResolution) => Object.fromEntries(block.refs.map(({ path, ...rest }: RefEntry) => [path, rest]));
/** The printed text of each span segment, by its fields (or "•"). */
function segments(text: string, block: BlockResolution): string[] {
  assert.ok("certificate" in block.spans, JSON.stringify(block.spans));
  const bytes = Buffer.from(text);
  assert.equal(block.spans.certificate.bytes, bytes.length);
  return block.spans.certificate.segments.map(s => `${bytes.subarray(s.start, s.end).toString()}=${"fields" in s ? s.fields.join(",") : "•"}`);
}

describe("closed reference vocabulary", () => {
  it("resolves you by the block's controller and targets by announcement slot", () => {
    const [spell] = blocks("Target player draws two cards. You lose 2 life. Destroy target creature.", "Sorcery");
    assert.deepEqual(refs(spell), {
      "/effects/0/steps/0/who": { ref: { ref: "target", slot: 0 } },
      "/effects/1/steps/0/who": { ref: { ref: "controller", role: "spell-controller" } },
      "/effects/2/steps/0/what": { ref: { ref: "target", slot: 1 } },
    });
    assert.deepEqual(spell.residuals, []);
    const [, activated] = blocks("Flying\n{T}: You gain 1 life.", "Creature — Bird");
    assert.deepEqual(refs(activated)["/effects/0/steps/0/who"], { ref: { ref: "controller", role: "ability-controller" } });
  });

  it("keeps the source, the trigger's event object and attachments distinct", () => {
    const [dies] = blocks("When this creature dies, it deals 1 damage to any target.", "Creature — Goblin");
    assert.deepEqual(refs(dies)["/effects/0/steps/0/source"], { ref: { ref: "event-object", role: "dies" } });
    const [, aura] = blocks("Enchant creature\nEnchanted creature gets +2/+2.", "Enchantment — Aura");
    assert.deepEqual(Object.values(refs(aura)).filter(r => "ref" in r && r.ref.ref === "attached-to").length, 1);
  });

  it("binds later pronouns to earlier step results and defined numbers to their binding", () => {
    const [search] = blocks("Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.", "Sorcery");
    const put = Object.entries(refs(search)).find(([path]) => /steps\/1\/what$/.test(path));
    assert.ok(put && "ref" in put[1] && put[1].ref.ref === "step-result", JSON.stringify(refs(search)));
    const [defined] = blocks("You gain X life, where X is the number of creatures you control.", "Instant");
    assert.ok(Object.values(refs(defined)).some(r => "ref" in r && r.ref.ref === "bound-var" && r.ref.id === "/effects/0/bindings/0"), JSON.stringify(refs(defined)));
    const [announced] = blocks("Fixture Card deals X damage to any target.", "Instant", "{X}{R}");
    assert.ok(Object.values(refs(announced)).some(r => "ref" in r && r.ref.ref === "announced-x"), JSON.stringify(refs(announced)));
  });

  it("names unresolved antecedents as residuals instead of guessing", () => {
    const [unbound] = blocks("Draw that many cards.", "Sorcery");
    assert.deepEqual(refs(unbound)["/effects/0/steps/0/amount"], { unresolved: "missing-antecedent" });
    assert.ok(unbound.residuals.includes("ref:missing-antecedent"));
  });
});

describe("step obligations", () => {
  it("events printed steps under their discriminant and scopes structural ones", () => {
    const [modal] = blocks("Choose one —\n• Draw a card.\n• You gain 3 life.", "Instant");
    assert.deepEqual(modal.steps.map(({ path, ...rest }) => [path, rest]), [
      ["/effects/0/steps/0", { astEffect: "modal", scope: "member-steps" }],
      ["/effects/0/steps/0/options/0/effects/0/steps/0", { astEffect: "draw", event: "draw" }],
      ["/effects/0/steps/0/options/1/effects/0/steps/0", { astEffect: "gain-life", event: "gain-life" }],
    ]);
  });
});

describe("span certificates", () => {
  it("tiles every printed token with UTF-8 byte offsets", () => {
    const text = "Landfall — Whenever a land you control enters, you gain 1 life. Draw a card.";
    const [block] = blocks(text, "Creature — Elf");
    assert.deepEqual(segments(text, block), ["Landfall —=/abilityWord", "Whenever a land you control enters,=/trigger",
      "you gain 1 life.=/effects/0", "Draw a card.=/effects/1"]);
    // "—" is three UTF-8 bytes: the trigger starts at byte 13, not UTF-16 offset 11.
    assert.ok("certificate" in block.spans && block.spans.certificate.segments[1].start === 13);
  });

  it("points folded riders, activation rules and modal bullets at their fields", () => {
    const destroy = "Destroy target creature. It can't be regenerated.";
    assert.deepEqual(segments(destroy, blocks(destroy, "Instant")[0]),
      ["Destroy target creature.=/effects/0", "It can't be regenerated.=/effects/0/steps/0/regenerationForbidden"]);
    const restricted = "{2}, {T}: Draw a card. Activate only as a sorcery.";
    assert.deepEqual(segments(restricted, blocks(restricted, "Artifact")[0]),
      ["{2}, {T}:=/costs,/restriction", "Draw a card.=/effects/0", "Activate only as a sorcery.=/costs,/restriction"]);
    const modal = "Choose two —\n• Destroy all artifacts.\n• Destroy all enchantments.";
    assert.deepEqual(segments(modal, blocks(modal, "Sorcery")[0]), ["Choose two —=/effects/0", "•=•",
      "Destroy all artifacts.=/effects/0/steps/0/options/0", "•=•", "Destroy all enchantments.=/effects/0/steps/0/options/1"]);
  });

  it("refuses a certificate when a block was not parsed with source tokens", () => {
    const result = parseOracleText("Draw a card.", "Fixture Card", { typeLine: "Instant" });
    assert.ok(result.ok);
    const input = { identity: "fixture", source: { name: "Fixture Card", layout: "normal", type_line: "Instant" }, faces: [{ sourceFaceIndex: 0, result }] };
    const [block] = resolveCard(input, analyzeCardBindings(input)).faces[0].blocks!;
    assert.deepEqual(block.spans, { unsupported: "block has no recorded source tokens" });
    assert.ok(block.residuals.includes("spans:block has no recorded source tokens"));
  });
});

describe("kernel event players", () => {
  const eventPlayers = (block: BlockResolution) => block.refs.flatMap(entry =>
    "ref" in entry && entry.ref.ref === "event-player" ? [entry.ref.role] : []);

  it("names the player whose step began the active player, whoever's step the trigger names", () => {
    for (const whose of ["each player's", "each opponent's"]) {
      const [upkeep] = blocks(`At the beginning of ${whose} upkeep, that player loses 1 life.`, "Enchantment");
      assert.deepEqual(eventPlayers(upkeep), ["active-player"], JSON.stringify(upkeep.refs));
      assert.deepEqual(upkeep.residuals, []);
    }
  });

  it("names the caster in the kernel vocabulary", () => {
    const [cast] = blocks("Whenever an opponent casts a spell, that player loses 1 life.", "Enchantment");
    assert.deepEqual(eventPlayers(cast), ["caster"], JSON.stringify(cast.refs));
  });

  it("keeps the trigger discriminant for an event player the kernel does not name", () => {
    const [draws] = blocks("Whenever an opponent draws a card, that player loses 1 life.", "Enchantment");
    assert.deepEqual(eventPlayers(draws), ["draws"], JSON.stringify(draws.refs));
  });
});

describe("announced counter division (CR 601.2c-d)", () => {
  const charm = "Choose one —\n• Exile target creature with power 3 or greater.\n• You draw two cards and you lose 2 life.\n• Distribute two +1/+1 counters among one or two target creatures.";

  it("binds the chosen target count, the division and its recipients at announcement", () => {
    const result = parseOracleText(charm, "Fixture Charm", { typeLine: "Instant", layout: "normal", includeSourceProvenance: true });
    assert.ok(result.ok);
    const input = { identity: "fixture", source: { name: "Fixture Charm", layout: "normal", type_line: "Instant", mana_cost: "{W}{B}{G}" }, faces: [{ sourceFaceIndex: 0, result }] };
    const analysis = analyzeCardBindings(input);
    assert.deepEqual(analysis.diagnostics, []);
    const step = "/faces/0/abilities/0/effects/0/steps/0/options/2/effects/0/steps/0";
    const count = analysis.nodes.find(node => node.node === "target-count" && node.path === `${step}/among/count`);
    assert.ok(count?.node === "target-count");
    assert.deepEqual({ minimum: count.minimum, maximum: count.maximum }, { minimum: 1, maximum: 2 });
    const event = analysis.nodes.find(node => node.node === "event" && node.path === step);
    assert.ok(event?.node === "event" && event.operation === "distribute-counters");
    const division = analysis.nodes.find(node => node.node === "target-division");
    assert.ok(division?.node === "target-division", JSON.stringify(analysis.nodes));
    assert.equal(division.total, 2);
    assert.equal(division.event, event.id);
    assert.deepEqual(event.inputs, [division.targets]);
    const [block] = resolveCard(input, analysis).faces[0].blocks!;
    assert.deepEqual(block.residuals, []);
    assert.deepEqual(refs(block)["/effects/0/steps/0/options/2/effects/0/steps/0/among"], { ref: { ref: "target", slot: 1 } });
    assert.ok(block.steps.some(s => s.path === "/effects/0/steps/0/options/2/effects/0/steps/0" && "event" in s && s.event === "distribute-counters"));
  });

  it("leaves divisions without an announcement scope unsupported", () => {
    for (const text of [
      "Distribute four +1/+1 counters among any number of target creatures.",
      "Distribute X +1/+1 counters among one or two target creatures.",
      "Distribute three +1/+1 counters among any number of creatures you control.",
    ]) {
      const [block] = blocks(text, "Sorcery", "{X}{G}");
      assert.ok(block.residuals.includes("step:distribute-counters"), `${text}: ${JSON.stringify(block.residuals)}`);
    }
  });
});
