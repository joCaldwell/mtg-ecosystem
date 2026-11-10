import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { Amount, PlayerRef } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseAmount, parsePlayerRef } from "../src/parser/refs.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Counts of players (CR 102.2-102.3) and the life-change eligibility of a
// player (CR 119.3). Wordings of Priest of the Blessed Graf, Surveyor's
// Scope, Strefan Maurer Progenitor, Kaito Bane of Nightmares, Voice of Many,
// Gev Scaled Scorch, Rix Maadi Guildmage and Papalymo Totolymo.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number): Amount => ({ amount: "fixed", value });
const lostLife = { lifeChange: { change: "lost", during: "this-turn" } } as const;
const opponentsWho = (filter: object): PlayerRef => ({ player: "filtered", base: { player: "each-opponent" }, filter }) as PlayerRef;
const playersWho = (filter: object): PlayerRef => ({ player: "filtered", base: { player: "each-player" }, filter }) as PlayerRef;
const landsYouControl: Amount = { amount: "count", of: { types: ["land"], control: "you" } };
function amount(text: string, expected: Amount) {
  const c = new Cursor(lex(text));
  assert.deepEqual(parseAmount(c), expected, text);
  assert.ok(c.done(), `${text}: ${c.errorMessage()}`);
}
function noAmount(text: string) {
  const c = new Cursor(lex(text));
  const parsed = parseAmount(c);
  assert.ok(parsed === null || !c.done(), `${text} must not be one complete amount`);
}
const ability = (text: string, typeLine = "Sorcery") => {
  const result = parseOracleText(text, "Test Card", { typeLine });
  assert.equal(result.lines.length, 1);
  const line = result.lines[0];
  return line.ok ? JSON.parse(JSON.stringify(line.ability)) : null;
};

describe("player counts", () => {
  it("counts the players satisfying a printed eligibility clause", () => {
    amount("the number of opponents who control more lands than you", { amount: "player-count",
      of: opponentsWho({ controls: { filter: { types: ["land"] }, comparison: { op: "gt", value: landsYouControl } } }) });
    amount("the number of players who control at least two more lands than you", { amount: "player-count",
      of: playersWho({ controls: { filter: { types: ["land"] }, comparison: { op: "ge", value: { amount: "plus", a: landsYouControl, b: n(2) } } } }) });
    amount("the number of players who lost life this turn", { amount: "player-count", of: playersWho(lostLife) });
    amount("the number of opponents who have three or more poison counters", { amount: "player-count",
      of: opponentsWho({ counterConstraints: [{ counter: { type: "poison" }, comparison: { op: "ge", value: n(3) } }] }) });
  });

  it("keeps the count of a player's opponents a different amount", () => {
    amount("the number of opponents you have", { amount: "opponent-count", who: { player: "you" } });
    // A bare plural names no eligibility to count by.
    noAmount("the number of opponents");
    noAmount("the number of players");
    noAmount("the number of opponents who lost life last turn");
  });

  it("scales one instruction by the number of qualifying players", () => {
    assert.deepEqual(ability("Draw two cards for each opponent who controls fewer creatures than you."), {
      kind: "spell", effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: { amount: "product", factors: [n(2),
        { amount: "player-count", of: opponentsWho({ controls: { filter: { types: ["creature"] },
          comparison: { op: "lt", value: { amount: "count", of: { types: ["creature"], control: "you" } } } } }) }] } }] }],
    });
    assert.deepEqual(ability("At the beginning of your end step, create a Blood token for each player who lost life this turn.", "Creature — Vampire"), {
      kind: "triggered", trigger: { trigger: "phase", phase: "end-step", whose: "your" },
      effects: [{ steps: [{ effect: "create-token", count: { amount: "product", factors: [n(1), { amount: "player-count", of: playersWho(lostLife) }] },
        token: { predefined: "Blood" } }] }],
    });
    // The existing unit keeps its node.
    assert.deepEqual(ability("Draw a card for each opponent you have."), {
      kind: "spell", effects: [{ steps: [{ effect: "draw", who: { player: "you" },
        amount: { amount: "product", factors: [n(1), { amount: "opponent-count", who: { player: "you" } }] } }] }],
    });
  });

  it("multiplies entry counters by the number of qualifying players", () => {
    assert.deepEqual(ability("Other creatures you control enter with an additional +1/+1 counter on them for each opponent who lost life this turn.", "Creature — Lizard"), {
      kind: "static", effect: { static: "enters-with-counters", what: { ref: "each", filter: { other: true, types: ["creature"], control: "you" } },
        counter: { type: "+1/+1" }, count: n(1), additional: true, multiplier: { amount: "player-count", of: opponentsWho(lostLife) } },
    });
  });

  it("does not count targets, single players or unsupported periods", () => {
    assert.equal(ability("Draw a card for each target opponent who lost life this turn."), null);
    assert.equal(ability("Draw a card for each opponent."), null);
    assert.equal(ability("Draw a card for each player who lost life last turn."), null);
  });
});

describe("players who gained or lost life this turn", () => {
  const player = (text: string) => {
    const c = new Cursor(lex(text));
    const ref = parsePlayerRef(c);
    return ref && c.done() ? ref : null;
  };
  it("is an eligibility filter on the quantified or targeted player", () => {
    assert.deepEqual(player("each opponent who lost life this turn"), opponentsWho(lostLife));
    assert.deepEqual(player("each player who gained life this turn"), playersWho({ lifeChange: { change: "gained", during: "this-turn" } }));
    assert.deepEqual(ability("Target player who lost life this turn loses 1 life."), {
      kind: "spell", effects: [{ steps: [{ effect: "lose-life",
        who: { player: "filtered", base: { player: "target-player" }, filter: lostLife }, amount: n(1) }] }],
    });
  });
  it("rejects other periods and amounts instead of dropping them", () => {
    assert.equal(player("each opponent who lost life last turn"), null);
    assert.equal(player("each opponent who lost 3 or more life this turn"), null);
    assert.equal(player("each opponent who lost life"), null);
  });
});
