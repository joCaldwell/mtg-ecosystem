import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";
import { resolveCard, type BlockResolution, type RefEntry } from "../src/semantic/resolution.ts";

// Pool selections (Archive R5 RB-2). Oracle text is copied from the frozen
// corpus oracle-source-f8cb9da2…; the paired negatives keep the RFC guards.
// CR 608.2c (an instruction's later text refers to its earlier choices),
// 701.20a (reveal), 701.23a/b (search) and 400.7j:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt

function card(text: string, typeLine: string, manaCost = "{1}", name = "Fixture Card") {
  const result = parseOracleText(text, name, { typeLine, includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result.lines));
  const input = { identity: "fixture", source: { name, layout: "normal", type_line: typeLine, mana_cost: manaCost }, faces: [{ sourceFaceIndex: 0, result }] };
  const binding = analyzeCardBindings(input);
  return { binding, blocks: resolveCard(input, binding).faces[0].blocks! };
}
const refs = (block: BlockResolution) => Object.fromEntries(block.refs.map(({ path, ...rest }: RefEntry) => [path, rest]));
const selection = (step: string) => ({ ref: { ref: "step-result", step, role: "selection" } });
const remainder = (step: string) => ({ ref: { ref: "step-result", step, role: "remainder" } });

describe("library dig picks and the rest (B01)", () => {
  it("binds Harald's revealed pick and the rest of the looked-at cards", () => {
    const { binding, blocks } = card("Menace (This creature can't be blocked except by two or more creatures.)\nWhen Harald enters, look at the top five cards of your library. You may reveal an Elf, Warrior, or Tyvar card from among them and put it into your hand. Put the rest on the bottom of your library in a random order.",
      "Legendary Creature — Elf Warrior", "{1}{B}{G}", "Harald, King of Skemfar");
    const block = blocks[1];
    assert.deepEqual(block.residuals, []);
    const bound = refs(block);
    assert.deepEqual(bound["/effects/1/steps/0/steps/0/what/filter/fromAmong"], { ref: { ref: "step-result", step: "/effects/0/steps/0", role: "objects" } });
    assert.deepEqual(bound["/effects/1/steps/0/steps/1/what"], selection("/effects/1/steps/0/steps/0/what"));
    assert.deepEqual(bound["/effects/2/steps/0/what"], remainder("/effects/2/steps/0"));
    const rest = binding.nodes.find(node => node.node === "remainder");
    assert.ok(rest?.node === "remainder");
    assert.match(rest.pool, /\/effects\/0\/steps\/0#objects$/);
    assert.match(rest.excluding, /\/effects\/1\/steps\/0\/steps\/0\/what#query$/);
    // A declined reveal leaves the whole looked-at pool as the rest.
    assert.ok(binding.engineFeatures.some(need => need.feature === "optional-look-remainder"));
  });

  it("binds Commune with Dinosaurs the same way", () => {
    const { blocks: [block] } = card("Look at the top five cards of your library. You may reveal a Dinosaur or land card from among them and put it into your hand. Put the rest on the bottom of your library in any order.", "Sorcery", "{G}", "Commune with Dinosaurs");
    assert.deepEqual(block.residuals, []);
    assert.deepEqual(refs(block)["/effects/1/steps/0/steps/1/what"], selection("/effects/1/steps/0/steps/0/what"));
    assert.deepEqual(refs(block)["/effects/2/steps/0/what"], remainder("/effects/2/steps/0"));
  });

  it("binds an opponent's pick from your revealed cards and the rest (Murmurs from Beyond)", () => {
    const { blocks: [block] } = card("Reveal the top three cards of your library. An opponent chooses one of them. Put that card into your graveyard and the rest into your hand.", "Instant — Arcane", "{2}{U}", "Murmurs from Beyond");
    assert.deepEqual(block.residuals, []);
    assert.deepEqual(refs(block)["/effects/2/steps/0/steps/0/what"], selection("/effects/1/steps/0/what"));
    assert.deepEqual(refs(block)["/effects/2/steps/0/steps/1/what"], remainder("/effects/2/steps/0/steps/1"));
  });

  it("keeps the rest of another player's revealed cards unresolved (Lonis)", () => {
    const { blocks } = card("Whenever another nontoken creature you control enters, investigate.\n{T}, Sacrifice X Clues: Target opponent reveals the top X cards of their library. You may put a nonland permanent card with mana value X or less from among them onto the battlefield under your control. That player puts the rest on the bottom of their library in a random order.",
      "Legendary Creature — Snake Elf Scout", "{G}{U}", "Lonis, Cryptozoologist");
    assert.deepEqual(refs(blocks[1])["/effects/2/steps/0/what"], { unresolved: "unsupported-reference" });
  });

  it("keeps piles unresolved (Truth or Tale)", () => {
    const { blocks: [block] } = card("Reveal the top five cards of your library and separate them into two piles. An opponent chooses one of those piles. Put a card from the chosen pile into your hand, then put all other cards revealed this way on the bottom of your library in any order.", "Instant", "{1}{U}", "Truth or Tale");
    assert.deepEqual(refs(block)["/effects/2/steps/0/what/filter/fromAmong"], { unresolved: "unsupported-reference" });
    assert.ok("unresolved" in refs(block)["/effects/2/steps/1/what"]);
    assert.ok(!block.refs.some(ref => "ref" in ref && ref.ref.ref === "step-result" && ref.ref.role === "remainder"));
  });

  it("keeps the rest unresolved with two revealed pools or two picks", () => {
    const { blocks: [pools] } = card("Reveal the top three cards of your library. Target opponent reveals the top three cards of their library. You may put a creature card from among them into your hand. Put the rest into your graveyard.", "Sorcery");
    assert.deepEqual(refs(pools)["/effects/2/steps/0/what/filter/fromAmong"], { unresolved: "ambiguous-antecedent" });
    assert.deepEqual(refs(pools)["/effects/3/steps/0/what"], { unresolved: "unsupported-reference" });
    const { blocks: [picks] } = card("Look at the top five cards of your library. You may reveal a creature card from among them and put it into your hand. Put a land card from among them onto the battlefield. Put the rest into your graveyard.", "Sorcery");
    assert.deepEqual(refs(picks)["/effects/3/steps/0/what"], { unresolved: "unsupported-reference" });
  });

  it("keeps the rest unresolved when a second pick from the pool is itself unresolved (Wakanda Forever!)", () => {
    const { binding, blocks: [block] } = card("Reveal the top six cards of your library. You may put a permanent card from among them onto the battlefield with an indestructible counter on it. You may put a permanent card from among them into your hand. Put the rest into your graveyard.", "Sorcery", "{4}{G}{G}", "Wakanda Forever!");
    assert.deepEqual(refs(block)["/effects/3/steps/0/what"], { unresolved: "unsupported-reference" });
    assert.ok(!binding.nodes.some(node => node.node === "remainder"));
  });

  it("keeps the rest unresolved when another branch looks at a second pool", () => {
    const { binding, blocks: [block] } = card("Look at the top five cards of your library. If you control a creature, look at the top three cards of your library. Otherwise, put a card from among them into your hand. Put the rest on the bottom of your library in any order.", "Sorcery");
    assert.deepEqual(refs(block)["/effects/2/steps/0/what"], { unresolved: "unsupported-reference" });
    assert.ok(!binding.nodes.some(node => node.node === "remainder"));
  });

  it("counts every pick on every path, including the same-sentence \"one of them\" form", () => {
    const look = "Look at the top five cards of your library. ";
    const twoPicks = "Put one of them into your hand, put a land card from among those cards onto the battlefield, then put the rest into your graveyard.";
    const { blocks: [plain] } = card(look + twoPicks, "Sorcery");
    assert.deepEqual(refs(plain)["/effects/1/steps/2/what"], { unresolved: "unsupported-reference" });
    const { blocks: [modal] } = card(`Choose one —\n• ${look}${twoPicks}\n• You gain 3 life.`, "Sorcery");
    assert.deepEqual(refs(modal)["/effects/0/steps/0/options/0/effects/1/steps/2/what"], { unresolved: "unsupported-reference" });
    const { blocks: [three] } = card(look + "Put one of them into your hand, put a land card from among those cards onto the battlefield, put a card from among them with the chosen name into your graveyard, then put the rest on the bottom of your library in any order.", "Sorcery");
    assert.deepEqual(refs(three)["/effects/1/steps/3/what"], { unresolved: "unsupported-reference" });
    // One pick in the same sentence still binds (Impulse).
    const { blocks: [impulse] } = card("Look at the top four cards of your library. Put one of them into your hand and the rest on the bottom of your library in any order.", "Sorcery", "{1}{U}", "Impulse");
    assert.deepEqual(impulse.residuals, []);
    assert.deepEqual(refs(impulse)["/effects/1/steps/0/steps/1/what"], remainder("/effects/1/steps/0/steps/1"));
  });

  it("counts a reveal-until scan in another branch as a competing pool", () => {
    const { binding, blocks: [block] } = card("Look at the top five cards of your library. If you control a creature, reveal cards from the top of your library until you reveal a creature card. Otherwise, put a card from among them into your hand. Put the rest on the bottom of your library in any order.", "Sorcery");
    assert.deepEqual(refs(block)["/effects/2/steps/0/what"], { unresolved: "unsupported-reference" });
    assert.ok(!binding.nodes.some(node => node.node === "remainder"));
  });

  it("does not count a single revealed card or a re-reveal of the same cards as a pool", () => {
    const look = "Look at the top five cards of your library. ";
    const { blocks: [hand] } = card(look + "Put a creature card from among them into your hand. Reveal a card from your hand. Put the rest on the bottom of your library in any order.", "Sorcery");
    assert.deepEqual(hand.residuals, []);
    assert.deepEqual(refs(hand)["/effects/3/steps/0/what"], remainder("/effects/3/steps/0"));
    const { blocks: [again] } = card(look + "Reveal those cards. Put a creature card from among them into your hand. Put the rest on the bottom of your library in any order.", "Sorcery");
    assert.deepEqual(again.residuals, []);
    assert.deepEqual(refs(again)["/effects/3/steps/0/what"], remainder("/effects/3/steps/0"));
  });

  it("counts an unconditional reveal-until scan's revealed cards as a competing pool", () => {
    const { blocks: [block] } = card("Look at the top five cards of your library. Put up to two creature cards from among them onto the battlefield. Reveal cards from the top of your library until you reveal a creature card. Put the rest on the bottom of your library in any order.", "Sorcery");
    assert.deepEqual(refs(block)["/effects/3/steps/0/what"], { unresolved: "unsupported-reference" });
  });

  it("treats a later move from the pool's zone as an unaccounted pick", () => {
    const { blocks: [milled] } = card("Mill five cards. Exile a creature card from among them. Exile a card from your graveyard. Put the rest on the bottom of your library in any order.", "Sorcery");
    assert.deepEqual(refs(milled)["/effects/3/steps/0/what"], { unresolved: "unsupported-reference" });
    const look = "Look at the top five cards of your library. Put a creature card from among them into your hand. ";
    const { blocks: [top] } = card(look + "Exile the top card of your library. Put the rest on the bottom of your library in any order.", "Sorcery");
    assert.deepEqual(refs(top)["/effects/3/steps/0/what"], { unresolved: "unsupported-reference" });
    // A move from another zone is provably disjoint from the looked-at cards.
    const { blocks: [hand] } = card(look + "Exile a card from your hand. Put the rest on the bottom of your library in any order.", "Sorcery");
    assert.deepEqual(hand.residuals, []);
    assert.deepEqual(refs(hand)["/effects/3/steps/0/what"], remainder("/effects/3/steps/0"));
    // So is a permanent on the battlefield.
    const { blocks: [battlefield] } = card(look + "Sacrifice a creature. Put the rest on the bottom of your library in any order.", "Sorcery");
    assert.deepEqual(refs(battlefield)["/effects/3/steps/0/what"], remainder("/effects/3/steps/0"));
  });

  it("leaves the rest unresolved when the pick it excludes is unresolved (Forging the Anchor)", () => {
    const { blocks: [block] } = card("Look at the top five cards of your library. You may reveal any number of artifact cards from among them and put the revealed cards into your hand. Put the rest on the bottom of your library in a random order.", "Sorcery", "{2}{U}", "Forging the Anchor");
    assert.ok("unresolved" in refs(block)["/effects/2/steps/0/what"]);
    const { blocks: [tavern] } = card("Choose one —\n• Form a Party — Look at the top five cards of your library. You may reveal any number of creature cards from among them and put them into your hand. Put the rest on the bottom of your library in a random order.\n• Start a Brawl — Creatures you control get +2/+2 until end of turn.", "Sorcery", "{3}{G}", "You Meet in a Tavern");
    assert.ok("unresolved" in refs(tavern)["/effects/0/steps/0/options/0/effects/2/steps/0/what"]);
  });

  it("marks only a pick that may take nothing as leaving the whole pool", () => {
    // Declining the look skips both placements; given the look, the one-card pick is mandatory.
    const cream = card("Whenever a creature you control enters, you may look at the top X cards of your library, where X is that creature's power. If you do, put one of those cards on top of your library and the rest on the bottom of your library in any order.", "Enchantment", "{1}{G}", "Cream of the Crop");
    assert.deepEqual(cream.blocks[0].residuals, []);
    assert.deepEqual(cream.binding.engineFeatures, []);
    assert.equal(cream.binding.fullyBound, true);
    // "Up to one" may take nothing.
    const thicket = card("This land enters tapped.\nWhen this land enters, you may look at the top five cards of your library. If you do, reveal up to one basic land card from among them, then put that card on top of your library and the rest on the bottom in any order.\n{T}: Add {G}.", "Land", "", "Fertile Thicket");
    assert.deepEqual(thicket.blocks[1].residuals, []);
    assert.deepEqual(thicket.binding.engineFeatures.map(need => need.feature), ["optional-look-remainder"]);
  });

  it("binds the same-sentence form only for one proven player's pool", () => {
    const { blocks: [one] } = card("Target opponent reveals the top three cards of their library. Put one of them into your hand and the rest into their graveyard.", "Sorcery");
    assert.deepEqual(refs(one)["/effects/1/steps/0/steps/1/what"], remainder("/effects/1/steps/0/steps/1"));
    const { blocks: [each] } = card("Each opponent reveals the top three cards of their library. Put one of them into your hand and the rest into their graveyard.", "Sorcery");
    assert.deepEqual(refs(each)["/effects/1/steps/0/steps/1/what"], { unresolved: "unsupported-reference" });
  });

  it("keeps a searched pool's one-of-them remainder (Cultivate) and the rest in both branches (Nine-Fingers Keene)", () => {
    const cultivate = card("Search your library for up to two basic land cards, reveal those cards, put one onto the battlefield tapped and the other into your hand, then shuffle.", "Sorcery", "{2}{G}", "Cultivate");
    assert.deepEqual(cultivate.blocks[0].residuals, []);
    assert.ok(cultivate.binding.nodes.some(node => node.node === "remainder"));
    const keene = card("Menace\nWard\u2014Pay 9 life.\nWhenever Nine-Fingers Keene deals combat damage to a player, look at the top nine cards of your library. You may put a Gate card from among them onto the battlefield. Then if you control nine or more Gates, put the rest into your hand. Otherwise, put the rest on the bottom of your library in a random order.", "Legendary Creature \u2014 Human Rogue", "{1}{B}{G}{U}", "Nine-Fingers Keene");
    const remainders = keene.binding.nodes.filter(node => node.node === "remainder");
    assert.equal(remainders.length, 2);
    assert.ok(keene.blocks.every(block => !block.residuals.some(residual => residual.includes("unsupported-reference"))));
  });

  it("checks every arm of a compound selection against the pool's zone", () => {
    const { blocks: [block] } = card("Mill five cards. Exile a creature card from among them. Exile an artifact card or a creature card from your graveyard. Put the rest on the bottom of your library in any order.", "Sorcery");
    assert.deepEqual(refs(block)["/effects/3/steps/0/what"], { unresolved: "unsupported-reference" });
  });

  it("walks paths, not instruction order: exclusive branches and rows do not compete", () => {
    const look = "Look at the top five cards of your library. Put a creature card from among them into your hand. ";
    // The exile and the rest are on different branches either way round.
    const { blocks: [first] } = card(look + "If you control a creature, exile the top card of your library. Otherwise, put the rest on the bottom of your library in any order.", "Sorcery");
    assert.deepEqual(refs(first)["/effects/2/otherwise/steps/0/what"], remainder("/effects/2/otherwise/steps/0"));
    const { blocks: [second] } = card(look + "If you control a creature, put the rest on the bottom of your library in any order. Otherwise, exile the top card of your library.", "Sorcery");
    assert.deepEqual(refs(second)["/effects/2/steps/0/what"], remainder("/effects/2/steps/0"));
    // A conditional scan emits no event and shares a path with the rest.
    const { blocks: [scan] } = card(look + "If you control a creature, you may exile cards from the top of your library until you exile a land card. Put the rest on the bottom of your library in any order.", "Sorcery");
    assert.deepEqual(refs(scan)["/effects/3/steps/0/what"], { unresolved: "unsupported-reference" });
    // Druid of the Emerald Grove: the "9 or less" row's move cannot reach the "10-19" row's "the other".
    const druid = card("When this creature enters, search your library for up to two basic land cards and reveal them, then roll a d20.\n9 or less | Put those cards into your hand, then shuffle.\n10\u201419 | Put one of those cards onto the battlefield tapped and the other into your hand, then shuffle.\n20+ | Put those cards onto the battlefield tapped, then shuffle.", "Creature \u2014 Dwarf Druid", "{3}{G}", "Druid of the Emerald Grove");
    assert.ok(druid.binding.references.some(reference => "ref" in reference.expression && reference.expression.ref === "rest" && reference.binding === "bound"));
  });

  it("counts only the pick's own choice to take nothing", () => {
    const { binding, blocks: [block] } = card("You may look at the top five cards of your library, put one of them into your hand, and put the rest on the bottom of your library in any order.", "Sorcery");
    assert.deepEqual(block.residuals, []);
    assert.deepEqual(binding.engineFeatures, []);
  });

  it("compares the pool's actor and owner, and its zone, by identity", () => {
    const { blocks: [library] } = card("Target opponent reveals the top three cards of your library. Put one of them into your hand and the rest into your graveyard.", "Sorcery");
    assert.deepEqual(refs(library)["/effects/1/steps/0/steps/1/what"], { unresolved: "unsupported-reference" });
    const { blocks: [hand] } = card("Target opponent reveals their hand. Put one of them into your hand, exile a card from their hand, then put the rest into their graveyard.", "Sorcery");
    assert.deepEqual(refs(hand)["/effects/1/steps/2/what"], { unresolved: "unsupported-reference" });
  });

  it("drops the declined-choice feature from a remainder that is not established (Dihada)", () => {
    const { binding } = card("+2: Up to one target legendary creature gains vigilance, lifelink, and indestructible until your next turn.\n\u22123: Reveal the top four cards of your library. Put any number of legendary cards from among them into your hand and the rest into your graveyard. Create a Treasure token for each card put into your graveyard this way.\n\u221211: Gain control of all nonland permanents until end of turn. Untap them. They gain haste until end of turn.\nDihada, Binder of Wills can be your commander.", "Legendary Planeswalker \u2014 Dihada", "{1}{R}{W}{B}", "Dihada, Binder of Wills");
    assert.ok(!binding.engineFeatures.some(need => need.feature === "optional-look-remainder" &&
      binding.references.some(reference => reference.path === need.path && reference.binding !== "bound")));
  });

  it("keeps a plural pronoun after an any-number pick from a plural pool unresolved", () => {
    const { blocks: [block] } = card("Choose one —\n• Form a Party — Look at the top five cards of your library. You may reveal any number of creature cards from among them and put them into your hand. Put the rest on the bottom of your library in a random order.\n• Start a Brawl — Creatures you control get +2/+2 until end of turn.", "Sorcery", "{3}{G}", "You Meet in a Tavern");
    assert.deepEqual(refs(block)["/effects/0/steps/0/options/0/effects/1/steps/0/steps/1/what"], { unresolved: "ambiguous-antecedent" });
  });
});

describe("hand reveals and the chosen card (B02)", () => {
  it("binds Thoughtseize's revealed hand and the chosen card", () => {
    const { blocks: [block] } = card("Target player reveals their hand. You choose a nonland card from it. That player discards that card. You lose 2 life.", "Sorcery", "{B}", "Thoughtseize");
    assert.deepEqual(block.residuals, []);
    assert.deepEqual(refs(block)["/effects/1/steps/0/what/filter/fromAmong"], selection("/effects/0/steps/0/what"));
    assert.deepEqual(refs(block)["/effects/2/steps/0/what"], selection("/effects/1/steps/0/what"));
  });

  it("binds a looked-at hand (Painful Memories)", () => {
    const { blocks: [block] } = card("Look at target opponent's hand and choose a card from it. Put that card on top of that player's library.", "Sorcery", "{1}{B}", "Painful Memories");
    assert.deepEqual(block.residuals, []);
    assert.deepEqual(refs(block)["/effects/0/steps/1/what/filter/fromAmong"], selection("/effects/0/steps/0/what"));
    assert.deepEqual(refs(block)["/effects/1/steps/0/what"], selection("/effects/0/steps/1/what"));
  });

  it("never reads a second \"from it\" as the first chosen card", () => {
    const { blocks: [block] } = card("Target opponent reveals their hand. You choose a nonland card from it. You choose a land card from it. That player discards that card.", "Sorcery");
    assert.deepEqual(refs(block)["/effects/1/steps/0/what/filter/fromAmong"], selection("/effects/0/steps/0/what"));
    assert.deepEqual(refs(block)["/effects/2/steps/0/what/filter/fromAmong"], { unresolved: "ambiguous-antecedent" });
    // Two attempted picks: "that card" is not proven either.
    assert.deepEqual(refs(block)["/effects/3/steps/0/what"], { unresolved: "ambiguous-antecedent" });
  });

  it("keeps a choice from two zones unresolved (Memory Leak)", () => {
    const { blocks: [block] } = card("Target opponent reveals their hand. You choose a nonland card from that player's graveyard or hand and exile it.\nCycling {1} ({1}, Discard this card: Draw a card.)", "Sorcery", "{1}{B}", "Memory Leak");
    assert.deepEqual(refs(block)["/effects/1/steps/1/what"], { unresolved: "ambiguous-antecedent" });
  });
});

describe("searches of several of your own zones (B24)", () => {
  it("binds Finale of Devastation's search and links its searched-zone condition", () => {
    const { binding, blocks: [block] } = card("Search your library and/or graveyard for a creature card with mana value X or less and put it onto the battlefield. If you search your library this way, shuffle. If X is 10 or more, creatures you control get +X/+X and gain haste until end of turn.",
      "Sorcery", "{X}{G}{G}", "Finale of Devastation");
    assert.deepEqual(block.residuals, []);
    assert.deepEqual(refs(block)["/effects/0/steps/1/what"], selection("/effects/0/steps/0/for"));
    const link = binding.nodes.find(node => node.node === "event-link" && node.action === "search");
    assert.ok(link?.node === "event-link");
    assert.match(link.path, /\/effects\/1\/condition$/);
    assert.match(link.event, /\/effects\/0\/steps\/0#event$/);
  });

  it("keeps a search of another player's zones unsupported (Counterbore)", () => {
    const { binding } = card("Counter target spell. Search its controller's graveyard, hand, and library for all cards with the same name as that spell and exile them. Then that player shuffles.", "Instant", "{3}{U}{U}", "Counterbore");
    assert.ok(binding.diagnostics.some(diagnostic => diagnostic.code === "unsupported-scope" && diagnostic.path.endsWith("/effects/1/steps/0")));
  });
});
