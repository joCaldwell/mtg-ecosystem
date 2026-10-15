import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// CR 614.15 / 616.1a self-replacements and
// related replacement effects; wordings checked in the frozen corpus.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed' as const, value });
const you = { player: 'you' };
function abilities(text: string, typeLine = 'Sorcery', name?: string): unknown[] {
  const parsed = parseOracleText(text, name, { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities));
}
function fails(text: string, typeLine = 'Sorcery'): void {
  assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
}

describe('leading "instead" self-replacements', () => {
  it('reads "instead only you draw" as the replacement draw (Kumena\'s Awakening)', () => {
    assert.deepEqual(abilities("At the beginning of your upkeep, each player draws a card. If you have the city's blessing, instead only you draw a card.", 'Enchantment'), [{
      kind: 'triggered', trigger: { trigger: 'phase', phase: 'upkeep', whose: 'your' },
      effects: [{ steps: [{ effect: 'self-replacement',
        original: { effect: 'draw', who: { player: 'each-player' }, amount: n(1) },
        condition: { condition: 'designation', who: you, designation: "city's-blessing" },
        instead: [{ effect: 'draw', who: you, amount: n(1) }], targetPolicy: 'retain-original' }] }],
    }]);
  });
  it('replaces your optional instruction with your optional alternative (Court of Bounty)', () => {
    const fromHand = { cls: 'card', zone: { zone: 'hand', owner: 'your' } };
    assert.deepEqual(abilities("At the beginning of your upkeep, you may put a land card from your hand onto the battlefield. If you're the monarch, instead you may put a creature or land card from your hand onto the battlefield.", 'Enchantment'), [{
      kind: 'triggered', trigger: { trigger: 'phase', phase: 'upkeep', whose: 'your' },
      effects: [{ steps: [{ effect: 'self-replacement',
        original: { effect: 'move-zone', what: { ref: 'filter', filter: { types: ['land'], ...fromHand }, count: n(1) }, to: { zone: 'battlefield' }, optional: true },
        condition: { condition: 'designation', who: you, designation: 'monarch' },
        instead: [{ effect: 'move-zone', what: { ref: 'filter', filter: { allOf: [{ anyOf: [{ types: ['creature'] }, { types: ['land'] }] }], ...fromHand }, count: n(1) }, to: { zone: 'battlefield' } }],
        targetPolicy: 'retain-original', replacementChoice: { who: you, declined: 'original' } }] }],
    }]);
  });
  it('keeps an optional original unreplaced by a mandatory or trailing alternative', () => {
    fails("At the beginning of your upkeep, you may put a land card from your hand onto the battlefield. If you're the monarch, instead put a creature card from your hand onto the battlefield.", 'Enchantment');
    fails("At the beginning of your upkeep, you may put a land card from your hand onto the battlefield. If you're the monarch, you may put a creature card from your hand onto the battlefield instead.", 'Enchantment');
    fails("At the beginning of your upkeep, you may draw a card. If you're the monarch, instead target opponent may draw two cards.", 'Enchantment');
    fails("At the beginning of your upkeep, each player draws a card. If you're the monarch, only you draw a card instead.", 'Enchantment');
  });
});

describe('abbreviated and rider-extended self-replacements', () => {
  const horror = { colors: ['black'], supertypes: [], types: ['creature'], subtypes: ['Horror'], keywords: [], power: n(2), toughness: n(2), tapped: true };
  it('keeps the original token specification when the replacement defines its own X (The Final Days)', () => {
    assert.deepEqual(abilities('Create two tapped 2/2 black Horror creature tokens. If this spell was cast from a graveyard, instead create X of those tokens, where X is the number of creature cards in your graveyard.'), [{
      kind: 'spell', effects: [{ steps: [{ effect: 'self-replacement',
        original: { effect: 'create-token', count: n(2), token: horror },
        condition: { condition: 'was-cast', what: { ref: 'this', noun: 'spell' }, from: { zone: 'graveyard', owner: 'any' } },
        instead: { sequence: [{ steps: [{ effect: 'create-token', count: { amount: 'x' }, token: horror }],
          bindings: [{ variable: 'x', value: { amount: 'count', of: { types: ['creature'], cls: 'card', zone: { zone: 'graveyard', owner: 'your' } } } }] }] },
        targetPolicy: 'retain-original' }] }],
    }]);
  });
  it('reads an abbreviated prevention budget without "this way" (Elvish Healer)', () => {
    const shield = (value: number) => ({ effect: 'prevent-damage', prevention: { eventScope: { sources: 'simultaneous', recipients: 'simultaneous' },
      shield: { shield: 'damage-budget', amount: n(value) }, kind: 'all', to: [{ ref: 'any-target' }] }, duration: { duration: 'this-turn' } });
    assert.deepEqual(abilities("{T}: Prevent the next 1 damage that would be dealt to any target this turn. If it's a green creature, prevent the next 2 damage instead.", 'Creature — Elf Cleric'), [{
      kind: 'activated', costs: [{ cost: 'tap-self' }], effects: [{ steps: [{ effect: 'self-replacement', original: shield(1),
        condition: { condition: 'is-filter', what: { ref: 'it' }, filter: { colors: ['green'], types: ['creature'] } },
        instead: [shield(2)], targetPolicy: 'retain-original' }] }],
    }]);
    fails("{T}: Prevent the next 1 damage that would be dealt to any target this turn. If it's a green creature, prevent the next 2 damage to you instead.", 'Creature — Elf Cleric');
  });
  it('rejects a definition after an abbreviation of anything but the token count', () => {
    fails('Create two tapped 2/2 black Horror creature tokens. If this spell was cast from a graveyard, instead create X of those tokens, where X is the number of creature cards in your graveyard, then draw a card.');
    fails('Draw two cards. If this spell was cast from a graveyard, instead draw X cards, where X is the number of creature cards in your graveyard, and you lose 2 life.');
  });
  it('folds a regeneration rider into the leading-instead replacement (Toxic Stench)', () => {
    assert.deepEqual(abilities("Target nonblack creature gets -1/-1 until end of turn.\nThreshold — If there are seven or more cards in your graveyard, instead destroy that creature. It can't be regenerated.", 'Instant'), [{
      kind: 'spell', effects: [{ steps: [{ effect: 'self-replacement',
        original: { effect: 'pump', what: { ref: 'target', filter: { nonColors: ['black'], types: ['creature'] }, count: n(1) },
          power: { sign: -1, amount: n(1) }, toughness: { sign: -1, amount: n(1) }, duration: { duration: 'end-of-turn' } },
        condition: { condition: 'count', of: { cls: 'card', zone: { zone: 'graveyard', owner: 'your' } }, comparison: { op: 'ge', value: n(7) } },
        instead: [{ effect: 'destroy', what: { ref: 'that', filter: { types: ['creature'] } }, regenerationForbidden: true }],
        targetPolicy: 'retain-original', abilityWord: 'Threshold' }] }],
    }]);
  });
  it('does not fold an independent later sentence into the replacement', () => {
    assert.deepEqual(abilities("Target creature gets -1/-1 until end of turn. If you control a Swamp, instead destroy that creature. You draw a card.", 'Instant'), [{
      kind: 'spell', effects: [
        { steps: [{ effect: 'self-replacement',
          original: { effect: 'pump', what: { ref: 'target', filter: { types: ['creature'] }, count: n(1) },
            power: { sign: -1, amount: n(1) }, toughness: { sign: -1, amount: n(1) }, duration: { duration: 'end-of-turn' } },
          condition: { condition: 'control', who: you, what: { ref: 'filter', filter: { subtypes: ['Swamp'] }, count: n(1) } },
          instead: [{ effect: 'destroy', what: { ref: 'that', filter: { types: ['creature'] } } }],
          targetPolicy: 'retain-original' }] },
        { steps: [{ effect: 'draw', who: you, amount: n(1) }] },
      ],
    }]);
  });
});

describe('replacing a spell\'s move to the graveyard as it resolves', () => {
  const instantOrSorcery = { allOf: [{ anyOf: [{ types: ['instant'] }, { types: ['sorcery'] }] }], cls: 'spell' };
  const affected = { ref: 'replacement-event-object', role: 'affected' };
  it('creates a resolution-only replacement with the exile counters (Goliath Daydreamer)', () => {
    assert.deepEqual(abilities('Whenever you cast an instant or sorcery spell from your hand, exile that card with a dream counter on it instead of putting it into your graveyard as it resolves.', 'Creature — Giant Wizard'), [{
      kind: 'triggered', trigger: { trigger: 'cast', who: you, what: instantOrSorcery, from: { zone: 'hand', owner: 'your' } },
      effects: [{ steps: [{ effect: 'create-replacement', replacement: {
        event: { event: 'move-zone', what: { ref: 'that', filter: { cls: 'card' } }, to: { zone: 'graveyard', owner: 'your' }, resolving: true },
        occurrence: 'each',
        replace: { replacement: 'effects', effects: [{ steps: [{ effect: 'exile', what: affected, counters: [{ counter: { type: 'dream' }, count: n(1) }] }] }] } } }] }],
    }]);
  });
  it('watches any graveyard for "a graveyard" (Rod of Absorption)', () => {
    assert.deepEqual(abilities('Whenever a player casts an instant or sorcery spell, exile it instead of putting it into a graveyard as it resolves.', 'Artifact'), [{
      kind: 'triggered', trigger: { trigger: 'cast', who: { player: 'a-player' }, what: instantOrSorcery },
      effects: [{ steps: [{ effect: 'create-replacement', replacement: {
        event: { event: 'move-zone', what: { ref: 'it' }, to: { zone: 'graveyard', owner: 'any' }, resolving: true },
        occurrence: 'each',
        replace: { replacement: 'effects', effects: [{ steps: [{ effect: 'exile', what: affected }] }] } } }] }],
    }]);
  });
  it('continues the replacement with the rest of the ability (Lilah, Undefeated Slickshot)', () => {
    assert.deepEqual(abilities('Whenever you cast a multicolored instant or sorcery spell from your hand, exile that spell instead of putting it into your graveyard as it resolves. If you do, it becomes plotted.', 'Legendary Creature — Human Rogue'), [{
      kind: 'triggered', trigger: { trigger: 'cast', who: you, what: { multicolored: true, ...instantOrSorcery }, from: { zone: 'hand', owner: 'your' } },
      effects: [{ steps: [{ effect: 'create-replacement', replacement: {
        event: { event: 'move-zone', what: { ref: 'that', filter: { cls: 'spell' } }, to: { zone: 'graveyard', owner: 'your' }, resolving: true },
        occurrence: 'each',
        replace: { replacement: 'effects', effects: [
          { steps: [{ effect: 'exile', what: affected }] },
          { steps: [{ effect: 'become-status', what: { ref: 'it' }, status: 'plotted' }], condition: { condition: 'you-do' }, conditionKind: 'if' },
        ] } } }] }],
    }]);
    fails('Whenever you cast an instant spell, exile it instead of putting it into your graveyard as it resolves. You gain 1 life.', 'Artifact');
  });
  it('rejects other replaced destinations, timings, or replaced objects', () => {
    fails('Whenever you cast an instant spell, exile it instead of putting it into your graveyard.', 'Artifact');
    fails('Whenever you cast an instant spell, exile it instead of putting it into your hand as it resolves.', 'Artifact');
    fails('Whenever you cast an instant spell, exile target card instead of putting it into your graveyard as it resolves.', 'Artifact');
    fails('Whenever you cast an instant spell, you may exile it instead of putting it into your graveyard as it resolves.', 'Artifact');
  });
  it('exiles "that card" with counters instead of reading the counters as a requirement', () => {
    assert.deepEqual(abilities('If a nontoken creature would die, exile that card with a blood counter on it instead.', 'Creature — Vampire'), [{
      kind: 'static', effect: { static: 'replacement', replacement: {
        event: { event: 'move-zone', what: { ref: 'filter', filter: { nonToken: true, types: ['creature'] }, count: n(1) }, from: { zone: 'battlefield' }, to: { zone: 'graveyard' } },
        occurrence: 'each',
        replace: { replacement: 'effects', effects: [{ steps: [{ effect: 'exile', what: affected, counters: [{ counter: { type: 'blood' }, count: n(1) }] }] }] } } },
    }]);
  });
});

describe('static modification replaced while a condition holds', () => {
  const pt = (power: number, toughness: number) => ({ modifier: 'pt', power: { sign: power < 0 ? -1 : 1, amount: n(Math.abs(power)) }, toughness: { sign: toughness < 0 ? -1 : 1, amount: n(Math.abs(toughness)) } });
  it('makes the first sentence the otherwise branch (Precipitous Drop)', () => {
    const what = { ref: 'enchanted', noun: 'creature' };
    assert.deepEqual(abilities("Enchanted creature gets -2/-2. It gets -5/-5 instead as long as you've completed a dungeon.", 'Enchantment — Aura'), [{
      kind: 'static', effect: { static: 'conditional', condition: { condition: 'completed-dungeon', who: you },
        effect: { static: 'modify', what, modifiers: [pt(-5, -5)] },
        otherwise: { static: 'modify', what, modifiers: [pt(-2, -2)] } },
    }]);
  });
  it('rejects an additional modification, another condition form, or a plural subject', () => {
    fails("Enchanted creature gets -2/-2. It gets -5/-5 as long as you've completed a dungeon.", 'Enchantment — Aura');
    fails("Enchanted creature gets -2/-2. It gets -5/-5 instead if you've completed a dungeon.", 'Enchantment — Aura');
    fails("Enchanted creature gets -2/-2 as long as you control a Swamp. It gets -5/-5 instead as long as you've completed a dungeon.", 'Enchantment — Aura');
    fails("Creatures you control get +1/+1. It gets +2/+2 instead as long as you've completed a dungeon.", 'Enchantment');
  });
});

describe('replacement coordinated with a preceding instruction', () => {
  it('shares the sentence condition with the rest of the sentence (Disintegrate)', () => {
    const it_ = { ref: 'it' };
    assert.deepEqual(abilities("Disintegrate deals X damage to any target. If it's a creature, it can't be regenerated this turn, and if it would die this turn, exile it instead.", 'Sorcery', 'Disintegrate'), [{
      kind: 'spell', effects: [
        { steps: [{ effect: 'damage', source: { ref: 'self' }, amount: { amount: 'x' }, to: { to: 'any-target' } }] },
        { steps: [
          { effect: 'cant', what: it_, action: 'be-regenerated', duration: { duration: 'this-turn' } },
          { effect: 'create-replacement', replacement: {
            event: { event: 'move-zone', what: it_, from: { zone: 'battlefield' }, to: { zone: 'graveyard' } }, occurrence: 'each',
            replace: { replacement: 'effects', effects: [{ steps: [{ effect: 'exile', what: { ref: 'replacement-event-object', role: 'affected' } }] }] } },
            duration: { duration: 'this-turn' } },
        ], condition: { condition: 'is-filter', what: it_, filter: { types: ['creature'] } }, conditionKind: 'if' },
      ],
    }]);
  });
  it('requires a complete replacement after "and if"', () => {
    fails("Disintegrate deals X damage to any target. If it's a creature, it can't be regenerated this turn, and if it would die this turn, exile it.", 'Sorcery');
    fails("Disintegrate deals X damage to any target. If it's a creature, it can't be regenerated this turn, and if you control a Swamp, exile it.", 'Sorcery');
  });
});

describe('conjunctive control conditions', () => {
  const control = (colors: string[]) => ({ condition: 'control', who: you, what: { ref: 'filter', filter: { colors, cls: 'permanent' }, count: n(1) } });
  it('tests each indefinite object separately (Necra Sanctuary)', () => {
    assert.deepEqual(abilities('At the beginning of your upkeep, if you control a green or white permanent, target player loses 1 life. If you control a green permanent and a white permanent, that player loses 3 life instead.', 'Enchantment'), [{
      kind: 'triggered', trigger: { trigger: 'phase', phase: 'upkeep', whose: 'your' },
      condition: { condition: 'control', who: you, what: { ref: 'filter', filter: { allOf: [{ anyOf: [{ colors: ['green'] }, { colors: ['white'] }] }], cls: 'permanent' }, count: n(1) } },
      effects: [{ steps: [{ effect: 'self-replacement',
        original: { effect: 'lose-life', who: { player: 'target-player' }, amount: n(1) },
        condition: { condition: 'all-of', conditions: [control(['green']), control(['white'])] },
        instead: [{ effect: 'lose-life', who: { player: 'that-player' }, amount: n(3) }], targetPolicy: 'retain-original' }] }],
    }]);
  });
  it('does not join a definite or counted object', () => {
    fails('If you control two Forests and a Plains, draw a card.');
    fails('If you control a Forest and that creature, draw a card.');
  });
});
describe('conjunctive control conditions (disjunction)', () => {
  it('does not group a disjunction with a conjunction', () => {
    fails('If you control a Plains or an Island and a Swamp, draw a card.');
  });
});
