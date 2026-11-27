import assert from "node:assert/strict";
import { test } from "node:test";
import { buildCardNameCatalog, parseOracleTextWithPrintedEvidence } from "../src/index.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { finalizeCharacteristicDefinitions } from "../src/parser/characteristic-origin.ts";
import { locateNodes } from "../src/printed/nodes.ts";
import { PrintedCapture } from "../src/printed/capture.ts";
import type { ByteRange, PrintedBlock, PrintedFace, PrintedFact, TokenRun } from "../src/printed/schema.ts";

type ReadBlock = Extract<PrintedBlock, { status: "read" }>;
type ReadFace = Extract<PrintedFace, { status: "read" }>;

function read(text: string, name: string, typeLine: string): ReadFace {
  const report = parseOracleTextWithPrintedEvidence(text, name, { typeLine });
  assert.equal(report.printed.status, "read");
  return report.printed;
}

function block(face: ReadFace, index = 0): ReadBlock {
  const found = face.blocks[index];
  assert.equal(found.status, "read", JSON.stringify(found));
  return found;
}

function originalRange(face: ReadFace, [line, start, end]: TokenRun): ByteRange {
  const row = face.lines[line];
  assert.equal(row.status, "lexed");
  return [row.tokens[start].o[0], row.tokens[end - 1].o[1]];
}

function spelling(face: ReadFace, text: string, span: TokenRun): string {
  const [start, end] = originalRange(face, span);
  return Buffer.from(text).toString("utf8", start, end);
}

function span(found: ReadBlock, path: string, basis: "recorded" | "hull" = "recorded"): TokenRun {
  const node = found.nodes.find(node => node.path === path);
  assert.ok(node, path);
  assert.equal(node.basis, basis, path);
  assert.equal(node.span.length, 1, path);
  return node.span[0];
}

function fact<K extends PrintedFact["fact"]>(found: ReadBlock, kind: K,
  match: (candidate: Extract<PrintedFact, { fact: K }>) => boolean = () => true) {
  const picked = found.facts.find((candidate): candidate is Extract<PrintedFact, { fact: K }> =>
    candidate.fact === kind && match(candidate as Extract<PrintedFact, { fact: K }>));
  assert.ok(picked, kind);
  return picked;
}

function assertLocated(face: ReadFace): void {
  for (const found of face.blocks) {
    assert.equal(found.status, "read");
    assert.deepEqual(found.nodes.filter(node => node.basis === "unlocated"), []);
  }
}

function folds(face: ReadFace, text: string, found: ReadBlock, path: string): string[] {
  const node = found.nodes.find(node => node.path === path);
  assert.equal(node?.basis, "recorded", path);
  return (node.folds ?? []).map(run => spelling(face, text, run));
}

const step = "/effects/0/steps/0";

test("failed cursor branches and manual seeks discard marks, including sliced children", () => {
  const tokens = lex("You draw three cards.", "line:0");
  const capture = new PrintedCapture();
  tokens.forEach((token, index) => capture.register(token, { line: 0, index }));
  const c = capture.run(() => new Cursor(tokens));
  const actor = { player: "you" } as const;
  const amount = { amount: "fixed", value: 3 } as const;
  assert.equal(c.attempt(c => {
    c.word("you");
    c.mark(actor, 0);
    const child = capture.run(() => new Cursor(c.tokens.slice(2)));
    child.attempt(child => { child.word("three"); return amount; });
    return null;
  }), null);
  assert.equal(c.pos, 0);
  assert.equal(capture.origin(actor), undefined);
  assert.equal(capture.origin(amount), undefined);
  assert.equal(c.attempt(c => {
    assert.equal(c.word("you"), "you");
    return actor;
  }), actor);
  assert.deepEqual(capture.origin(actor), { basis: "recorded", tokens: [{ line: 0, index: 0 }] });
  c.pos = 0;
  assert.equal(capture.origin(actor), undefined);

  const accepted = c.attempt(c => {
    assert.equal(c.word("you"), "you");
    const who = c.mark(actor, 0);
    c.word("draw");
    c.word("three");
    c.mark(amount, 2);
    return { kind: "spell", effects: [{ steps: [{ effect: "draw", who, amount }] }] } as const;
  })!;
  const final = capture.run(() => finalizeCharacteristicDefinitions({ kind: "spell", effects: [{ steps: [{ effect: "draw", who: actor, amount }] }] }));
  assert.equal(final.kind, "spell");
  const draw = final.effects[0].steps[0];
  assert.equal(draw.effect, "draw");
  assert.notEqual(draw.who, actor);
  assert.equal(capture.origin(draw.who), capture.origin(actor));
  assert.equal(capture.origin(draw.amount), capture.origin(amount));
  assert.ok(capture.origin(accepted));
});

test("R5:1344 Junk Diver records the dying creature, another target, and singular default", () => {
  const text = "Flying\nWhen this creature dies, return another target artifact card from your graveyard to your hand.";
  const parse = (source: string) => read(source, "Junk Diver", "Artifact Creature — Bird");
  const face = parse(text);
  assertLocated(face);
  const trigger = block(face, 1);
  assert.equal(spelling(face, text, span(block(face), "/keywords/0")), "Flying");
  assert.deepEqual(originalRange(face, span(trigger, "/trigger/what")), [12, 25]);
  assert.equal(spelling(face, text, span(trigger, "/trigger")), "this creature dies");
  const another = fact(trigger, "exception", f => f.form === "another");
  assert.deepEqual(originalRange(face, another.tok), [39, 46]);
  assert.equal(another.field, step + "/what/filter/other");
  assert.deepEqual(originalRange(face, span(trigger, step + "/what/filter/zone")), [73, 87]);
  const selected = fact(trigger, "target");
  assert.deepEqual(originalRange(face, selected.tok), [47, 53]);
  assert.equal(selected.targetIndex, 0);
  assert.equal(selected.selectionId, "line:1:7");
  assert.deepEqual(trigger.nodes.find(node => node.path === step + "/what/count"),
    { path: step + "/what/count", basis: "implicit", reason: "implied-singular-target" });

  const changed = block(parse(text.replace("another target", "target")), 1);
  assert.equal(changed.facts.some(f => f.fact === "exception" && f.form === "another"), false);
  assert.equal(fact(changed, "target").selectionId, "line:1:6");
});

test("R5:3602 Terastodon keeps optionality, one bounded introduction, and result mentions at their occurrences", () => {
  const text = "When this creature enters, you may destroy up to three target noncreature permanents. For each permanent put into a graveyard this way, its controller creates a 3/3 green Elephant creature token.";
  const parse = (source: string) => read(source, "Terastodon", "Creature — Elephant");
  const face = parse(text);
  assertLocated(face);
  const body = block(face);
  const may = fact(body, "optional", f => f.form === "may");
  assert.deepEqual(originalRange(face, may.tok), [31, 34]);
  assert.equal(may.field, step + "/optional");
  const limit = fact(body, "optional", f => f.form === "up-to");
  assert.equal(limit.field, step + "/what/upTo");
  assert.deepEqual(originalRange(face, span(body, step + "/what/count")), [49, 54]);
  assert.deepEqual(originalRange(face, fact(body, "target").tok), [55, 61]);
  assert.equal(body.facts.filter(f => f.fact === "target").length, 1);
  const non = fact(body, "polarity", f => f.form === "non-prefix");
  assert.deepEqual(non.n, [62, 65]);
  assert.equal(non.field, step + "/what/filter/nonTypes");
  const creation = "/effects/1/steps/0/effects/0/steps/0";
  const way = fact(body, "mention", f => f.form === "this-way");
  assert.deepEqual(originalRange(face, way.tok), [126, 134]);
  assert.equal(way.field, "/effects/1/steps/0/each/filter/relation/scope");
  assert.deepEqual(originalRange(face, span(body, creation + "/who")), [136, 150]);
  const power = span(body, creation + "/token/power");
  const toughness = span(body, creation + "/token/toughness");
  assert.equal(spelling(face, text, power), "3");
  assert.equal(spelling(face, text, toughness), "3");
  assert.equal(power[2] + 1, toughness[1]);

  const different = text.replace("you may destroy up to three target noncreature", "destroy up to two target creature");
  const changed = block(parse(different));
  assert.equal(changed.facts.some(f => f.fact === "optional" && f.form === "may"), false);
  assert.equal(changed.facts.some(f => f.fact === "polarity" && f.form === "non-prefix"), false);
  assert.equal(fact(changed, "quantity", f => f.field === step + "/what/count/value").value, 2);
});

test("R5:7870 Secret Rendezvous records the two recipient roles and the per-player amount", () => {
  const text = "You and target opponent each draw three cards.";
  const parse = (source: string) => read(source, "Secret Rendezvous", "Sorcery");
  const face = parse(text);
  assertLocated(face);
  const body = block(face);
  assert.deepEqual(originalRange(face, span(body, step + "/who/members/0")), [0, 3]);
  assert.deepEqual(originalRange(face, span(body, step + "/who/members/1")), [8, 23]);
  assert.deepEqual(originalRange(face, fact(body, "introducer", f => f.form === "each").tok), [24, 28]);
  assert.deepEqual(originalRange(face, span(body, step + "/amount")), [34, 39]);
  assert.equal(fact(body, "target").field, step + "/who/members/1/selectionId");
  assert.equal(fact(body, "mention", f => f.form === "you").field, step + "/who/members/0/player");

  const changed = block(parse("Target opponent draws three cards."));
  assert.ok(changed.nodes.every(node => !node.path.includes("/members/")));
  assert.equal(changed.facts.some(f => f.fact === "mention" && f.form === "you"), false);
  assert.equal(fact(changed, "target").field, step + "/who/selectionId");
});

test("R5:8135 and :37759 keep mana production, restriction and rider spans disjoint", () => {
  const boseiju = "Boseiju enters tapped.\n{T}, Pay 2 life: Add {C}. If that mana is spent on an instant or sorcery spell, that spell can't be countered.";
  const halfling = "{T}: Add {C}.\n{T}: Add one mana of any color. Spend this mana only to cast a legendary spell, and that spell can't be countered.";
  const land = (source: string) => read(source, "Boseiju, Who Shelters All", "Legendary Land");
  const creature = (source: string) => read(source, "Delighted Halfling", "Creature — Halfling Citizen");
  const riderPath = step + "/riders/0";
  const restrictionPath = step + "/restriction";
  for (const [source, face] of [[boseiju, land(boseiju)], [halfling, creature(halfling)]] as const) {
    assertLocated(face);
    const activation = block(face, 1);
    const production = span(activation, "/effects/0");
    const rider = span(activation, riderPath, "hull");
    assert.equal(production[0], rider[0]);
    assert.ok(production[2] <= rider[1]);
    assert.equal(spelling(face, source, span(activation, "/costs/0")), "{T}");
    assert.equal(spelling(face, source, fact(activation, "polarity", f => f.form === "can't").tok), "can't");
    assert.ok(block(face).nodes.every(node => node.path !== riderPath));
  }
  const full = land(boseiju);
  assert.deepEqual(full.edits[0], { edit: "self-name", form: "before-comma", o: [0, 7], n: [0, 1] });
  const conditional = block(full, 1);
  const filter = riderPath + "/spentOn/0/filter/allOf/0/anyOf/";
  assert.equal(spelling(full, boseiju, span(conditional, filter + "0")), "instant");
  assert.equal(spelling(full, boseiju, span(conditional, filter + "1")), "sorcery");
  assert.deepEqual(conditional.nodes.find(node => node.path === step + "/who"),
    { path: step + "/who", basis: "implicit", reason: "imperative-controller" });
  assert.equal(spelling(full, boseiju, span(conditional, riderPath, "hull")),
    "If that mana is spent on an instant or sorcery spell, that spell can't be countered.");
  const bare = block(land(boseiju.slice(0, boseiju.indexOf(" If"))), 1);
  assert.ok(bare.nodes.every(node => node.path !== riderPath));

  const split = creature(halfling);
  const activation = block(split, 1);
  const restriction = span(activation, restrictionPath);
  assert.ok(span(activation, "/effects/0")[2] <= restriction[1]);
  assert.ok(restriction[2] <= span(activation, riderPath, "hull")[1]);
  assert.equal(spelling(split, halfling, restriction), "Spend this mana only to cast a legendary spell");
  assert.deepEqual(originalRange(split, fact(activation, "polarity", f => f.form === "only").tok), [62, 66]);
  assert.deepEqual(originalRange(split, span(activation, riderPath + "/effects/0/steps/0/what")), [98, 108]);
  assert.deepEqual(activation.nodes.find(node => node.path === riderPath + "/spentOn/0"),
    { path: riderPath + "/spentOn/0", basis: "alias", of: restrictionPath + "/uses/0" });
  const noRider = block(creature(halfling.replace(", and that spell can't be countered", "")), 1);
  assert.ok(noRider.nodes.every(node => node.basis !== "alias" && node.path !== riderPath));
  assert.ok(noRider.nodes.some(node => node.path === restrictionPath && node.basis === "recorded"));
});

test("target introductions exclude a trigger mention and a removed reminder", () => {
  const text = "Whenever this creature becomes the target of a spell, destroy target creature. (Choose any target.)";
  const face = read(text, "Test", "Creature");
  const body = block(face);
  const target = fact(body, "target");
  assert.equal(target.targetIndex, 0);
  assert.equal(spelling(face, text, target.tok), "target");
  assert.equal(body.facts.filter(f => f.fact === "target").length, 1);
  assert.equal(body.facts.filter(f => f.fact === "mention" && f.form === "target").length, 1);
  assert.equal(face.removedReminders.length, 1);
  assert.equal(Buffer.from(text).toString("utf8", ...face.removedReminders[0].o), "(Choose any target.)");
});

test("R5:516 station suffix retains physical locations and cites the removed threshold", () => {
  const text = "When this Spacecraft enters, create a 2/2 colorless Robot artifact creature token.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike";
  const face = read(text, "Wedgelight Rammer", "Artifact — Spacecraft");
  assertLocated(face);
  const band = block(face, 2);
  const location = span(band, "/effect/abilities/0/keywords/0");
  assert.deepEqual(location, [2, 3, 4]);
  assert.equal(spelling(face, text, location), "Flying");
  const citation = fact(band, "reminder-citation");
  assert.equal(citation.field, "/effect/creature/basePT");
  assert.equal(citation.reminderIndex, 0);
  assert.equal(Buffer.from(text).toString("utf8", ...citation.o), "It's an artifact creature at 9+.");
  assert.equal(read(text.replace("at 9+", "at 8+"), "Wedgelight Rammer", "Artifact — Spacecraft").blocks[2].status, "failed");
});

test("generated literal quote anchors stay off the wire and literal tokens keep the original hull", () => {
  const cardNames = buildCardNameCatalog([{ name: "Squadron Hawk", layout: "normal" }]);
  for (const name of ["Squadron Hawk", "“Squadron Hawk”"]) {
    const text = `Search your library for a card named ${name}.`;
    const { printed } = parseOracleTextWithPrintedEvidence(text, "Test", { cardNames, typeLine: "Sorcery" });
    assert.equal(printed.status, "read");
    assert.equal("generatedQuotes" in printed, false);
    const line = printed.lines[0];
    assert.equal(line.status, "lexed");
    const literal = line.tokens.find(token => token.t === "card-name");
    assert.ok(literal);
    assert.equal(Buffer.from(text).toString("utf8", ...literal.o), name);
  }
});

test("target numbering separates quoted abilities and continues across modal options", () => {
  const text = 'Target creature gains "{T}: Tap target creature." until end of turn. Untap target creature.';
  const body = block(read(text, "Test", "Instant"));
  const targets = body.facts.filter(fact => fact.fact === "target");
  assert.deepEqual(targets.map(target => [target.targetIndex, target.scope]), [
    [0, ""], [0, step + "/abilities/0/ability"], [1, ""],
  ]);

  const modal = block(read("Choose one —\n• Destroy target creature.\n• Untap target land.", "Test", "Instant"));
  assert.deepEqual(modal.facts.filter(fact => fact.fact === "target").map(target => [target.targetIndex, target.selectionId]),
    [[0, "line:1:2"], [1, "line:2:2"]]);
  assert.deepEqual(span(modal, step), [0, 0, 3]);
});

test("quote settling preserves physical punctuation order and failed blocks keep byte diagnostics", () => {
  const text = 'Equipped creature has "{T}, Unattach Leonin Bola: Tap target creature."';
  const face = read(text, "Leonin Bola", "Artifact — Equipment");
  const punctuation = block(face).facts.filter(fact => fact.fact === "punct").filter(fact => fact.moved);
  assert.deepEqual(punctuation.map(fact => spelling(face, text, fact.tok)), [".", '"']);
  const line = face.lines[0];
  assert.equal(line.status, "lexed");
  for (const fact of punctuation) assert.notEqual(line.tokens[fact.tok[1]].grammarIndex, fact.tok[1]);

  const failed = read("Draw a card.\n@", "Test", "Sorcery").blocks[1];
  assert.equal(failed.status, "failed");
  assert.deepEqual(failed.n, [13, 14]);
  assert.equal(failed.at, 13);
  assert.equal(failed.location, "lex-error");
});


test("the mutation guard keeps only child or folded spans after an uncovered scalar change", () => {
  const tokens = lex("Destroy target creature. It can't be regenerated.", "line:0");
  const capture = new PrintedCapture();
  tokens.forEach((token, index) => capture.register(token, { line: 0, index }));
  const c = capture.run(() => new Cursor(tokens));
  const what = c.mark({ ref: "target" }, 1, 3);
  const destroy = { effect: "destroy", what, regenerationForbidden: false };
  c.mark(destroy, 0, 3);
  destroy.regenerationForbidden = true;

  const audit = () => {
    const defects = { nesting: 0, overlap: 0, conflict: 0, unlocatedSelections: 0, mutationGuardDemotions: 0 };
    const nodes = locateNodes(destroy, capture, [0, 1], defects);
    return { root: nodes[0].node, demotions: defects.mutationGuardDemotions };
  };

  assert.deepEqual(audit(), { root: { path: "", basis: "hull", span: [[0, 1, 3]] }, demotions: 1 });
  c.fold(destroy, 4, tokens.length);
  assert.deepEqual(audit(), { root: { path: "", basis: "recorded", span: [[0, 0, 3]], folds: [[0, 4, tokens.length]] }, demotions: 0 });

  const amount = { amount: "fixed", value: 1 };
  c.mark(amount, 1, 2);
  amount.value = 2;
  const defects = { nesting: 0, overlap: 0, conflict: 0, unlocatedSelections: 0, mutationGuardDemotions: 0 };
  assert.deepEqual(locateNodes(amount, capture, [0, 1], defects)[0].node, { path: "", basis: "unlocated" });
  assert.equal(defects.mutationGuardDemotions, 1);
});

test("Polymorph and Pongify attach the regeneration sentence to the destruction", () => {
  const instruction = "Destroy target creature. It can't be regenerated.";
  const examples = [
    ["Polymorph", "Sorcery", instruction + " Its controller reveals cards from the top of their library until they reveal a creature card. The player puts that card onto the battlefield, then shuffles all other cards revealed this way into their library."],
    ["Pongify", "Instant", instruction + " Its controller creates a 3/3 green Ape creature token."],
  ];
  for (const [name, type, text] of examples) {
    const face = read(text, name, type);
    const body = block(face);
    assert.equal(spelling(face, text, span(body, step)), "Destroy target creature");
    assert.deepEqual(folds(face, text, body, step), ["It can't be regenerated."]);
  }
  const clause = "Destroy target creature and it can't be regenerated.";
  const face = read(clause, "Test", "Instant");
  assert.deepEqual(folds(face, clause, block(face), step), ["and it can't be regenerated"]);
});

test("unpreventable damage folds supported riders and retains Banefire's failed paragraph", () => {
  const combust = "This spell can't be countered.\nCombust deals 5 damage to target white or blue creature. The damage can't be prevented.";
  const direct = read(combust, "Combust", "Instant");
  assert.deepEqual(folds(direct, combust, block(direct, 1), step), ["The damage can't be prevented."]);

  const rage = "Kicker {8}{R} (You may pay an additional {8}{R} as you cast this spell.)\nThis spell can't be countered.\nUrza's Rage deals 3 damage to any target. If this spell was kicked, instead it deals 10 damage to that permanent or player and the damage can't be prevented.";
  const kicked = read(rage, "Urza's Rage", "Instant");
  assert.deepEqual(folds(kicked, rage, block(kicked, 2), step + "/instead/0"), ["and the damage can't be prevented"]);

  const banefire = "Banefire deals X damage to any target.\nIf X is 5 or more, this spell can't be countered and the damage can't be prevented.";
  const unsupported = read(banefire, "Banefire", "Sorcery");
  assert.deepEqual(folds(unsupported, banefire, block(unsupported), step), []);
  assert.equal(unsupported.blocks[1].status, "failed");
});

test("an explicit optional prefix belongs to the action or to its enclosing group", () => {
  const direct = "You may draw a card.";
  const solo = read(direct, "Test", "Sorcery");
  assert.deepEqual(folds(solo, direct, block(solo), step), ["You may"]);
  assert.equal(spelling(solo, direct, span(block(solo), step + "/who")), "You");

  const coordinated = "You may draw a card and discard a card.";
  const group = read(coordinated, "Test", "Sorcery");
  const body = block(group);
  assert.equal(spelling(group, coordinated, span(body, step)), coordinated.slice(0, -1));
  assert.equal(spelling(group, coordinated, span(body, step + "/steps/0")), "draw a card");
  assert.equal(spelling(group, coordinated, span(body, step + "/steps/1")), "discard a card");
});

test("coordinated characteristic modifiers keep their own spans and one shared duration", () => {
  const text = "Target creature gets +2/+2 and gains flying until end of turn.";
  const face = read(text, "Test", "Instant");
  const body = block(face);
  const second = "/effects/0/steps/1";
  assert.equal(spelling(face, text, span(body, step, "hull")), "Target creature gets +2/+2");
  assert.equal(spelling(face, text, span(body, second)), "gains flying");
  assert.deepEqual(folds(face, text, body, second), ["until end of turn"]);
  assert.deepEqual(body.nodes.find(node => node.path === step + "/duration"),
    { path: step + "/duration", basis: "alias", of: second + "/duration" });
});


test("printed actors and zones have local spans while a bare shuffle records both defaults", () => {
  const implicit = block(read("Shuffle.", "Test", "Sorcery"));
  assert.deepEqual(implicit.nodes.find(node => node.path === step + "/who"),
    { path: step + "/who", basis: "implicit", reason: "imperative-controller" });
  assert.deepEqual(implicit.nodes.find(node => node.path === step + "/library"),
    { path: step + "/library", basis: "implicit", reason: "default-zone" });
  const explicit = "You shuffle your library.";
  const shuffle = read(explicit, "Test", "Sorcery");
  assert.equal(spelling(shuffle, explicit, span(block(shuffle), step + "/who")), "You");
  assert.equal(spelling(shuffle, explicit, span(block(shuffle), step + "/library")), "your library");
});

test("printed counts, zones, and cost adjustments retain their own token spans", () => {
  for (const [text, typeLine, path, expected] of [
    ["Return target creature card from your graveyard to the battlefield.", "Sorcery", step + "/what/filter/zone", "your graveyard"],
    ["Return target creature card from your graveyard to the battlefield.", "Sorcery", step + "/to", "the battlefield"],
    ["Look at the top card of your library.", "Sorcery", step + "/what/top", "card"],
    ["Each opponent sacrifices a creature of their choice.", "Sorcery", step + "/who", "Each opponent"],
    ["Each opponent sacrifices a creature of their choice.", "Sorcery", step + "/what/count", "a"],
    ["You get {E}{E}.", "Sorcery", step + "/count", "{E}{E}"],
    ["+1: Draw a card.", "Legendary Planeswalker — Test", "/cost", "+1"],
    ["This spell costs {2} less to cast for each creature you control.", "Creature", "/effect/change", "{2} less"],
  ]) {
    const face = read(text, "Test", typeLine);
    assert.equal(spelling(face, text, span(block(face), path)), expected, path);
  }
});
