import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';
import { Cursor } from '../src/parser/cursor.ts';
import { lex } from '../src/lexer.ts';
import { parseDelayedPrefix, parseDelayedSuffix } from '../src/parser/delayed.ts';
import { parseSentences } from '../src/parser/effects.ts';

// Full Oracle sources from the frozen corpus. CR603.7b,614.10a preserve the
// specified turn/phase; additional steps in other turns cannot satisfy it.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const you = { player: 'you' };
const n = (value: number) => ({ amount: 'fixed', value });
const draw = { effect: 'draw', who: you, amount: n(1) };
const nextUpkeep = { trigger: 'phase-in-turn', phase: 'upkeep', turn: { turn: 'next' } };
function abilities(text: string, name?: string, typeLine?: string) {
  const result = parseOracleText(text, name, { typeLine, includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}

describe('delayed abilities tied to particular turns or combats', () => {
  it('retains Heal prevention and next-turn draw as distinct complete instructions', () => {
    assert.deepEqual(abilities("Prevent the next 1 damage that would be dealt to any target this turn.\nDraw a card at the beginning of the next turn's upkeep.", 'Heal', 'Instant'), [
      { kind: 'spell', effects: [{ steps: [{ effect: 'prevent-damage', prevention: {
        eventScope: { sources: 'simultaneous', recipients: 'simultaneous' }, shield: { shield: 'damage-budget', amount: n(1) },
        kind: 'all', to: [{ ref: 'any-target', selectionId: 'line:0:11' }],
      }, duration: { duration: 'this-turn' } }] }] },
      { kind: 'spell', effects: [{ steps: [{ effect: 'create-delayed-trigger', trigger: nextUpkeep, occurrence: 'next', nextOnly: true,
        effects: [{ steps: [draw] }], targets: 'enclosing-ability' }] }] },
    ]);
  });
  it('keeps Mishra’s Bauble immediate information target separate from its delayed draw', () => {
    assert.deepEqual(abilities("{T}, Sacrifice this artifact: Look at the top card of target player's library. Draw a card at the beginning of the next turn's upkeep.", "Mishra's Bauble", 'Artifact'), [{
      kind: 'activated', costs: [{ cost: 'tap-self' }, { cost: 'sacrifice', what: { ref: 'this', noun: 'artifact' } }], effects: [
        { steps: [{ effect: 'look-at', who: you, what: { top: n(1), of: { zone: 'library', owner: { player: 'target-player', selectionId: 'line:0:12' } } } }] },
        { steps: [{ effect: 'create-delayed-trigger', trigger: nextUpkeep, occurrence: 'next', nextOnly: true, effects: [{ steps: [draw] }], targets: 'enclosing-ability' }] },
      ],
    }]);
  });
  it('preserves Final Fortune linked extra turn without pretending its owner is the turn identity', () => {
    assert.deepEqual(abilities("Take an extra turn after this one. At the beginning of that turn's end step, you lose the game.", 'Final Fortune', 'Instant'), [{ kind: 'spell', effects: [
      { steps: [{ effect: 'add-turns', who: you, count: n(1), after: { turn: 'current' } }] },
      { steps: [{ effect: 'create-delayed-trigger', trigger: { trigger: 'phase-in-turn', phase: 'end-step', turn: { turn: 'that', scope: 'contextual' } },
        occurrence: 'next', nextOnly: true, effects: [{ steps: [{ effect: 'lose-game', who: you }] }], targets: 'delayed-ability' }] },
    ] }]);
  });
  it('retains World at War exact second-main anchor and particular added combat', () => {
    assert.deepEqual(abilities("After the second main phase this turn, there's an additional combat phase followed by an additional main phase. At the beginning of that combat, untap all creatures that attacked this turn.\nRebound (If you cast this spell from your hand, exile it as it resolves. At the beginning of your next upkeep, you may cast this card from exile without paying its mana cost.)", 'World at War', 'Sorcery'), [
      { kind: 'spell', effects: [
        { steps: [{ effect: 'add-phases', after: { phase: 'ordinal', kind: 'main', ordinal: 2, turn: 'current' }, sequence: [{ phase: 'combat', count: n(1) }, { phase: 'main', count: n(1) }] }] },
        { steps: [{ effect: 'create-delayed-trigger', trigger: { trigger: 'phase-instance', phase: { phase: 'that', kind: 'combat', scope: 'contextual' }, boundary: 'beginning' },
          occurrence: 'next', nextOnly: true, effects: [{ steps: [{ effect: 'untap', what: { ref: 'all', filter: { types: ['creature'], combatHistory: { action: 'attacked', during: 'this-turn' } } } }] }], targets: 'delayed-ability' }] },
      ] }, { kind: 'keywords', keywords: [{ keyword: 'rebound' }] },
    ]);
  });
  it('distinguishes a next step from a step within the next turn and retains explicit turn ownership', () => {
    for (const [suffix, trigger] of [
      ['your next upkeep', { trigger: 'phase', phase: 'upkeep', whose: 'your' }],
      ["your next turn's upkeep", { trigger: 'phase-in-turn', phase: 'upkeep', turn: { turn: 'next', whose: you } }],
      ["the next turn's upkeep", nextUpkeep],
    ]) assert.deepEqual(abilities(`Draw a card at the beginning of ${suffix}.`), [{ kind: 'spell', effects: [{ steps: [{
      effect: 'create-delayed-trigger', trigger, occurrence: 'next', nextOnly: true, effects: [{ steps: [draw] }], targets: 'enclosing-ability',
    }] }] }]);
  });
  it('preserves prefix and suffix target announcement with original source positions', () => {
    const prefix = abilities("At the beginning of your next turn's end step, destroy target creature.", undefined, 'Sorcery');
    assert.deepEqual(prefix, [{ kind: 'spell', effects: [{ steps: [{ effect: 'create-delayed-trigger',
      trigger: { trigger: 'phase-in-turn', phase: 'end-step', turn: { turn: 'next', whose: you } }, occurrence: 'next', nextOnly: true,
      effects: [{ steps: [{ effect: 'destroy', what: { ref: 'target', filter: { types: ['creature'] }, count: n(1), selectionId: 'line:0:11' } }] }], targets: 'delayed-ability',
    }] }] }]);
    const suffix = abilities("Destroy target creature at the beginning of the next turn's upkeep.");
    assert.deepEqual(suffix, [{ kind: 'spell', effects: [{ steps: [{ effect: 'create-delayed-trigger', trigger: nextUpkeep, occurrence: 'next', nextOnly: true,
      effects: [{ steps: [{ effect: 'destroy', what: { ref: 'target', filter: { types: ['creature'] }, count: n(1), selectionId: 'line:0:1' } }] }], targets: 'enclosing-ability',
    }] }] }]);
  });
  it('rolls back missing turn/step components and retains permanent spell rejection', () => {
    for (const text of ["At the beginning of the next turn's, draw a card.", "At the beginning of that turn's combat phase, draw a card.",
      'At the beginning of that combat phase, draw a card.', 'At the beginning of that combat, draw a card. Unsupported.']) {
      const c = new Cursor(lex(text));
      assert.equal(parseDelayedPrefix(c, { modalOptions: [] }, parseSentences), null, text);
      assert.equal(c.pos, 0, text);
    }
    const c = new Cursor(lex("at the beginning of the next turn's upkeep unexpectedly"));
    assert.equal(parseDelayedSuffix(c, [{ effect: 'draw', who: { player: 'you' }, amount: { amount: 'fixed', value: 1 } }]), null);
    assert.equal(c.pos, 0);
    assert.equal(parseOracleText("At the beginning of the next turn's upkeep, draw a card.", undefined, { typeLine: 'Enchantment' }).ok, false);
  });
});
