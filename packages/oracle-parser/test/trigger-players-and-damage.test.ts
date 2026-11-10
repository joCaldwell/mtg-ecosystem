import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { lex } from "../src/lexer.ts";
import { parseOracleText } from "../src/index.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseTrigger } from "../src/parser/triggers.ts";
import type { ObjectFilter, ObjectRef, Trigger } from "../src/ast.ts";

// Clauses are printed Oracle wording from the cached corpus (card named per
// case). Rules text: Comprehensive Rules 2026-09-25, cited per suite.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: "fixed", value }) as const;
const you = { player: "you" } as const;
const thisCreature: ObjectRef = { ref: "this", noun: "creature" };
const a = (filter: ObjectFilter): ObjectRef => ({ ref: "filter", filter, count: n(1) });

function trigger(text: string): Trigger {
  const c = new Cursor(lex(text));
  const result = parseTrigger(c);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true, `${text}: unconsumed tokens at ${c.pos}`);
  return result;
}

function unsupported(text: string, type = "Creature — Fish"): void {
  assert.equal(parseOracleText(`${text}, draw a card.`, "Fixture", { typeLine: type }).ok, false, text);
}

describe("beginning of combat step (CR 507)", () => {
  it("is the beginning of combat", () => {
    // Aggressive Crag; Occupation of Llanowar.
    assert.deepEqual(trigger("At the beginning of your combat step"), { trigger: "phase", phase: "combat", whose: "your" });
    assert.deepEqual(trigger("At the beginning of combat on your turn"), { trigger: "phase", phase: "combat", whose: "your" });
  });
});

describe("a player is attacked (CR 508.3b)", () => {
  it("is its own player event", () => {
    // Curse of Bounty.
    assert.deepEqual(trigger("Whenever enchanted player is attacked"), { trigger: "is-attacked", who: { player: "enchanted-player" } });
  });
  it("is not an attack declared by that player", () => {
    assert.notDeepEqual(trigger("Whenever enchanted player is attacked"), trigger("Whenever enchanted player attacks"));
  });
});

describe("damage thresholds and excess damage (CR 120.10)", () => {
  it("keeps the amount of damage in one event", () => {
    // Deus of Calamity; Dragonborn Champion; Spinneret and Spiderling; Innocent Bystander.
    assert.deepEqual(trigger("Whenever this creature deals 6 or more damage to an opponent"), {
      trigger: "deals-damage", what: thisCreature, combat: undefined, to: { player: "an-opponent" },
      amount: { op: "ge", value: n(6) },
    });
    assert.deepEqual(trigger("Whenever a source you control deals 5 or more damage to a player"), {
      trigger: "deals-damage", what: a({ cls: "source", control: "you" }), combat: undefined, to: "any",
      amount: { op: "ge", value: n(5) },
    });
    assert.deepEqual(trigger("Whenever ~ deals 4 or more damage"), {
      trigger: "deals-damage", what: { ref: "self" }, combat: undefined, to: undefined, amount: { op: "ge", value: n(4) },
    });
    assert.deepEqual(trigger("Whenever this creature is dealt 3 or more damage"), {
      trigger: "is-dealt-damage", what: thisCreature, amount: { op: "ge", value: n(3) },
    });
  });
  it("keeps the single-source requirement for a player", () => {
    // Pain Magnification.
    assert.deepEqual(trigger("Whenever an opponent is dealt 3 or more damage by a single source"), {
      trigger: "player-dealt-damage", who: { player: "an-opponent" }, amount: { op: "ge", value: n(3) }, singleSource: true,
    });
    unsupported("Whenever an opponent is dealt damage by a single source");
  });
  it("marks excess damage, optionally noncombat", () => {
    // Magmatic Galleon; Fall of Cair Andros.
    assert.deepEqual(trigger("Whenever one or more creatures your opponents control are dealt excess noncombat damage"), {
      trigger: "is-dealt-damage", what: a({ types: ["creature"], control: "opponent" }), combat: false, excess: true, grouping: "one-or-more",
    });
    assert.deepEqual(trigger("Whenever a creature an opponent controls is dealt excess damage"), {
      trigger: "is-dealt-damage", what: a({ types: ["creature"], control: "opponent" }), excess: true,
    });
    assert.notDeepEqual(trigger("Whenever a creature is dealt excess damage"), trigger("Whenever a creature is dealt damage"));
  });
  it("represents collective opponents being dealt damage as one occurrence", () => {
    // Master of Barbs.
    assert.deepEqual(trigger("Whenever one or more opponents are dealt noncombat damage"), {
      trigger: "player-dealt-damage", who: { player: "each-opponent" }, combat: false, grouping: "one-or-more",
    });
    assert.deepEqual(trigger("Whenever you're dealt damage"), { trigger: "player-dealt-damage", who: you });
  });
  it("fails near misses", () => {
    unsupported("Whenever this creature deals damage greater than 3 to an opponent");
    unsupported("Whenever one or more opponents are dealt 3 or more damage");
    unsupported("Whenever this creature is dealt excess");
  });
});

describe("another player as a trigger's player (CR 109.5)", () => {
  const other = { player: "filtered", base: { player: "a-player" }, filter: { otherThan: you } } as const;
  it("is a player other than the ability's controller", () => {
    // Aerial Extortionist; Night Dealings.
    assert.deepEqual(trigger("Whenever another player casts a spell from anywhere other than their hand"), {
      trigger: "cast", who: other, what: { cls: "spell" }, notFrom: { zone: "hand", owner: "their" },
    });
    assert.deepEqual(trigger("Whenever a source you control deals damage to another player"), {
      trigger: "deals-damage", what: a({ cls: "source", control: "you" }), combat: undefined, to: other,
    });
    assert.deepEqual(trigger("Whenever another player casts a spell"), trigger("Whenever a player other than you casts a spell"));
  });
});

describe("qualified damaged player and the player with the most life", () => {
  it("keeps the damaged player's qualifier", () => {
    // Cartographer's Hawk.
    const t = trigger("When this creature deals combat damage to a player who controls more lands than you");
    assert.equal(t.trigger, "deals-damage");
    assert.equal(t.trigger === "deals-damage" && typeof t.to === "object" && "player" in t.to && t.to.player, "filtered");
  });
  it("is the attacked player holding the highest life total", () => {
    // Undercover Butler; Seraphic Greatsword.
    assert.deepEqual(trigger("Whenever this creature attacks the player with the most life or tied for most life"), {
      trigger: "attacks", what: thisCreature, alone: undefined,
      whom: [{ player: "filtered", base: { player: "a-player" }, filter: { lifeTotal: { op: "eq", value: {
        amount: "life-total-extremum", extremum: "maximum", among: { player: "each-player" } } } } }],
    });
    // Without the tie clause it is the one player with the most life.
    assert.deepEqual(trigger("Whenever this creature attacks the player with the most life"), {
      trigger: "attacks", what: thisCreature, alone: undefined,
      whom: [{ player: "definite", filter: { lifeTotal: { op: "eq", value: {
        amount: "life-total-extremum", extremum: "maximum", among: { player: "each-player" } } } } }],
    });
  });
});

describe("during their turn, for the player performing the event", () => {
  it("restricts to the performing player's own turn", () => {
    // Unstable Glyphbridge; The Council of Four.
    assert.deepEqual(trigger("Whenever an opponent casts a spell during their turn"), {
      trigger: "cast", who: { player: "an-opponent" }, what: { cls: "spell" }, during: "event-player",
    });
    assert.deepEqual(trigger("Whenever a player draws their second card during their turn"), {
      trigger: "draws", who: { player: "a-player" }, occurrence: { ordinal: 2, period: "turn", per: "player" }, during: "event-player",
    });
    assert.notDeepEqual(trigger("Whenever an opponent casts a spell during their turn"), trigger("Whenever an opponent casts a spell during your turn"));
  });
  it("is not accepted for you or a non-player event", () => {
    unsupported("Whenever you cast a spell during their turn");
    unsupported("Whenever a creature dies during their turn");
  });
});

describe("upkeep costs, paying life and Saga chapters", () => {
  it("pays or declines the source's own upkeep cost (CR 702.24a, 702.30a)", () => {
    // Hibernation's End; Thought Lash; Balduvian Fallen; Shah of Naar Isle.
    assert.deepEqual(trigger("Whenever you pay this enchantment's cumulative upkeep"), {
      trigger: "pays-upkeep-cost", keyword: "cumulative-upkeep", what: { ref: "this", noun: "enchantment" }, who: you, paid: true,
    });
    assert.deepEqual(trigger("When a player doesn't pay this enchantment's cumulative upkeep"), {
      trigger: "pays-upkeep-cost", keyword: "cumulative-upkeep", what: { ref: "this", noun: "enchantment" }, who: { player: "a-player" }, paid: false,
    });
    assert.deepEqual(trigger("Whenever this creature's cumulative upkeep is paid"), {
      trigger: "pays-upkeep-cost", keyword: "cumulative-upkeep", what: thisCreature, paid: true,
    });
    assert.deepEqual(trigger("When this creature's echo cost is paid"), {
      trigger: "pays-upkeep-cost", keyword: "echo", what: thisCreature, paid: true,
    });
    unsupported("Whenever you pay another permanent's cumulative upkeep");
    unsupported("Whenever an opponent pays a tax from a spell or permanent you control");
  });
  it("pays life (CR 119.4)", () => {
    // Font of Agonies.
    assert.deepEqual(trigger("Whenever you pay life"), { trigger: "pays-life", who: you });
    unsupported("Whenever you pay 2 life");
  });
  it("names the final chapter ability of a Saga (CR 714.2e)", () => {
    // Historian's Boon; Narci, Fable Singer.
    assert.deepEqual(trigger("Whenever the final chapter ability of a Saga you control triggers"), {
      trigger: "final-chapter-ability", of: a({ subtypes: ["Saga"], control: "you" }), event: "triggers",
    });
    assert.deepEqual(trigger("Whenever the final chapter ability of a Saga you control resolves"), {
      trigger: "final-chapter-ability", of: a({ subtypes: ["Saga"], control: "you" }), event: "resolves",
    });
    unsupported("Whenever the final chapter ability of a creature you control resolves");
  });
});

describe("casting with mana from a source, or a spell targeting one or more", () => {
  it("requires mana produced by this source (CR 601.2g)", () => {
    // The Everflowing Well; Thousand Moons Smithy.
    assert.deepEqual(trigger("Whenever you cast a permanent spell using mana produced by ~"), {
      trigger: "cast", who: you, what: { cls: "spell", permanent: true }, usingManaFrom: { ref: "self" },
    });
    assert.deepEqual(trigger("Whenever you cast a permanent spell"), { trigger: "cast", who: you, what: { cls: "spell", permanent: true } });
    unsupported("Whenever you cast a permanent spell using mana produced by a Treasure");
  });
  it("keeps 'one or more' distinct from a single target", () => {
    // Storm, Windrider; Tiller of Flesh.
    assert.deepEqual(trigger("Whenever you cast a spell that targets one or more permanents"), {
      trigger: "cast", who: you, what: { cls: "spell", targets: { ref: "filter", filter: { cls: "permanent" }, count: { amount: "chosen", minimum: 1 } } },
    });
    assert.notDeepEqual(trigger("Whenever you cast a spell that targets one or more permanents"),
      trigger("Whenever you cast a spell that targets a permanent"));
  });
});

describe("a card called by a shortened name or by a lowercase article (CR 201.5c)", () => {
  const parsed = (text: string, name: string, typeLine: string) => {
    const result = parseOracleText(text, name, { typeLine });
    return result.ok ? JSON.parse(JSON.stringify(result.abilities)) : null;
  };
  it("reads the first word of a legendary name as the card when it is a trigger's subject", () => {
    // Hurska Sweet-Tooth; Legolas Greenleaf.
    const selfAttacks = [{
      kind: "triggered", trigger: { trigger: "attacks", what: { ref: "self" } },
      effects: [{ steps: [{ effect: "create-token", count: n(1), token: { predefined: "Food" } }] }],
    }];
    assert.deepEqual(parsed("Whenever Hurska attacks, create a Food token.", "Hurska Sweet-Tooth", "Legendary Creature — Bear"), selfAttacks);
    assert.deepEqual(parsed("Whenever ~ attacks, create a Food token.", "Hurska Sweet-Tooth", "Legendary Creature — Bear"), selfAttacks);
  });
  it("does not shorten other names", () => {
    // Not legendary; the first word is the card's own creature type; a title before a surname.
    assert.equal(parsed("Whenever Hurska attacks, create a Food token.", "Hurska Sweet-Tooth", "Creature — Bear"), null);
    assert.ok(!JSON.stringify(parsed("Whenever Sliver attacks, create a Food token.", "Sliver Overlord", "Legendary Creature — Sliver Mutant")).includes('"self"'));
    assert.equal(parsed("Whenever Captain Janeway attacks, create a Food token.", "Captain Kathryn Janeway", "Legendary Creature — Human"), null);
    assert.equal(parsed("Whenever Hurska attacks, create a Food token. Hurska Sweet-Tooth", "Hurska Sweet-Tooth", "Legendary Creature — Bear"), null);
  });
  it("replaces the full name when its article is lowercase", () => {
    // The Wizardly Barge.
    assert.deepEqual(parsed("Whenever the Wizardly Barge attacks, create a Food token.", "The Wizardly Barge", "Legendary Artifact — Vehicle"), [{
      kind: "triggered", trigger: { trigger: "attacks", what: { ref: "self" } },
      effects: [{ steps: [{ effect: "create-token", count: n(1), token: { predefined: "Food" } }] }],
    }]);
  });
});

describe("player keyword-action events", () => {
  it("names the action the player performs", () => {
    // Dream Devourer; Curator of Sun's Creation; Paranormal Analyst; Motion Sickness; Jolly Gerbils; Case File Auditor; Call of the Ring.
    assert.deepEqual(trigger("Whenever you foretell a card"), { trigger: "foretells", who: you });
    assert.deepEqual(trigger("Whenever you discover"), { trigger: "discovers", who: you });
    assert.deepEqual(trigger("Whenever you manifest dread"), { trigger: "manifests-dread", who: you });
    assert.deepEqual(trigger("Whenever you waterbend"), { trigger: "waterbends", who: you });
    assert.deepEqual(trigger("Whenever you open an Attraction"), { trigger: "opens-attraction", who: you });
    assert.deepEqual(trigger("Whenever you visit an Attraction"), { trigger: "visits-attraction", who: you });
    assert.deepEqual(trigger("Whenever you give a gift"), { trigger: "gives-gift", who: you });
    assert.deepEqual(trigger("Whenever you solve a Case"), { trigger: "solves-case", who: you });
    assert.deepEqual(trigger("Whenever you choose a creature as your Ring-bearer"), { trigger: "chooses-ring-bearer", who: you });
  });
  it("turns a permanent face up, as an actor", () => {
    // Growing Dread.
    assert.deepEqual(trigger("Whenever you turn a permanent face up"), {
      trigger: "turns-face-up", who: you, what: a({ cls: "permanent" }),
    });
    assert.notDeepEqual(trigger("Whenever you turn a permanent face up"), trigger("Whenever a permanent is turned face up"));
  });
  it("rejects wordings it does not represent", () => {
    unsupported("Whenever you foretell");
    unsupported("Whenever you solve");
    unsupported("Whenever you choose a creature as target player's Ring-bearer");
    unsupported("Whenever target player discovers");
  });
});

describe("every event but the first of its kind", () => {
  const spell = { cls: "spell" } as const;
  it("excludes the first matching event of the turn", () => {
    // Arcbound Tracker; Geralf, the Fleshwright; Ichneumon Druid.
    assert.deepEqual(trigger("Whenever you cast a spell other than your first spell each turn"), {
      trigger: "cast", who: you, what: spell, notOccurrence: { ordinal: 1, period: "turn" },
    });
    assert.deepEqual(trigger("Whenever you cast a spell during your turn other than your first spell that turn"), {
      trigger: "cast", who: you, what: spell, notOccurrence: { ordinal: 1, period: "turn" }, during: "your",
    });
    assert.deepEqual(trigger("Whenever an opponent casts an instant spell other than the first instant spell that player casts each turn"), {
      trigger: "cast", who: { player: "an-opponent" }, what: { types: ["instant"], cls: "spell" },
      notOccurrence: { ordinal: 1, period: "turn", per: "player" },
    });
    assert.notDeepEqual(trigger("Whenever you cast a spell other than your first spell each turn"),
      trigger("Whenever you cast your first spell each turn"));
  });
  it("applies the ordinal to the first alternative only", () => {
    // Curse of Shaken Faith.
    assert.deepEqual(trigger("Whenever enchanted player casts a spell other than the first spell they cast each turn or copies a spell"), {
      trigger: "any-of", events: [
        { trigger: "cast", who: { player: "enchanted-player" }, what: spell, notOccurrence: { ordinal: 1, period: "turn", per: "player" } },
        { trigger: "copy", who: { player: "enchanted-player" }, what: spell },
      ],
    });
  });
  it("fails when the restated description differs or the owner mismatches", () => {
    unsupported("Whenever you cast a creature spell other than your first instant spell each turn");
    unsupported("Whenever an opponent casts a spell other than your first spell each turn");
    unsupported("Whenever you cast a spell other than the first spell they cast each turn");
  });
});

describe("the Nth spell of a turn is cast, and casting a commander", () => {
  it("counts every player's matching spells together", () => {
    // Nullstone Gargoyle; Erayo, Soratami Ascendant.
    assert.deepEqual(trigger("Whenever the first noncreature spell of a turn is cast"), {
      trigger: "cast", who: { player: "a-player" }, what: { nonTypes: ["creature"], cls: "spell" }, occurrence: { ordinal: 1, period: "turn" },
    });
    assert.deepEqual(trigger("Whenever the fourth spell of a turn is cast"), {
      trigger: "cast", who: { player: "a-player" }, what: { cls: "spell" }, occurrence: { ordinal: 4, period: "turn" },
    });
    unsupported("Whenever the first creature of a turn is cast");
    unsupported("Whenever the fourth spell of a game is cast");
  });
  it("casts the player's commander card itself", () => {
    // Skyfire Phoenix.
    assert.deepEqual(trigger("When you cast your commander"), {
      trigger: "cast", who: you, what: { ref: "commander", owner: you, count: n(1) },
    });
  });
});
