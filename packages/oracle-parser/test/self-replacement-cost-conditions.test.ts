import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// Self-replacements conditioned on a cost chosen while casting: teamwork
// (CR 702.194b–c), the spell's own optional additional cost (118.8b, 601.2b),
// and alternative-cost keywords (emerge 702.119a, freerunning 702.173a,
// mayhem 702.187b, sneak 702.190a, madness 702.35b, spectacle 702.137a).
// Alternative targets are allowed only for costs chosen while casting
// (601.2c). Checked against the frozen Scryfall corpus: Cruel Alliance,
// Helicarrier Strike, Go Nuts!, Voltage Surge, Spirit Water Revival,
// Pyrrhic Strike, Monastery Raid, Rix Maadi Reveler, Sandman's Quicksand.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed' as const, value });
const you = { player: 'you' };
const thisSpell = { ref: 'this', noun: 'spell' };
const one = n(1);
function abilities(text: string, typeLine = 'Sorcery') {
  const parsed = parseOracleText(text, 'Test Card', { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities)) as unknown[];
}
const fails = (text: string, typeLine = 'Sorcery') => assert.equal(parseOracleText(text, 'Test Card', { typeLine }).ok, false, text);
const spell = (step: object) => [{ kind: 'spell', effects: [{ steps: [step] }] }];

describe('cost-choice conditions of self-replacements', () => {
  it('cast using teamwork, with alternative targets (Cruel Alliance)', () => {
    assert.deepEqual(abilities('Exile target creature with mana value 3 or less. If this spell was cast using teamwork, instead exile target creature and you gain 3 life.'),
      spell({ effect: 'self-replacement',
        original: { effect: 'exile', what: { ref: 'target', filter: { types: ['creature'], properties: [{ property: 'mana-value', comparison: { op: 'le', value: n(3) } }] }, count: one } },
        condition: { condition: 'keyword-choice', keyword: 'teamwork', what: thisSpell },
        instead: [{ effect: 'exile', what: { ref: 'target', filter: { types: ['creature'] }, count: one } },
          { effect: 'gain-life', who: you, amount: n(3) }],
        targetPolicy: 'alternative-by-cost' }));
  });
  it('the spell\'s optional additional cost was paid (Voltage Surge)', () => {
    const to = { to: 'object', ref: { ref: 'target', filter: { allOf: [{ anyOf: [{ types: ['creature'] }, { types: ['planeswalker'] }] }] }, count: one } };
    assert.deepEqual(abilities('~ deals 2 damage to target creature or planeswalker. If this spell\'s additional cost was paid, ~ deals 4 damage instead.', 'Instant'),
      spell({ effect: 'self-replacement', original: { effect: 'damage', source: { ref: 'self' }, amount: n(2), to },
        condition: { condition: 'additional-cost-paid', what: thisSpell },
        instead: [{ effect: 'damage', source: { ref: 'self' }, amount: n(4), to }], targetPolicy: 'retain-original' }));
    assert.deepEqual(abilities('Draw two cards. If this spell\'s additional cost was paid, instead draw seven cards.'),
      spell({ effect: 'self-replacement', original: { effect: 'draw', who: you, amount: n(2) },
        condition: { condition: 'additional-cost-paid', what: thisSpell },
        instead: [{ effect: 'draw', who: you, amount: n(7) }], targetPolicy: 'retain-original' }));
  });
  it('an alternative-cost keyword was paid (Monastery Raid, Rix Maadi Reveler)', () => {
    assert.deepEqual(abilities('Draw a card. If this spell\'s freerunning cost was paid, draw two cards instead.'),
      spell({ effect: 'self-replacement', original: { effect: 'draw', who: you, amount: one },
        condition: { condition: 'keyword-choice', keyword: 'freerunning', what: thisSpell },
        instead: [{ effect: 'draw', who: you, amount: n(2) }], targetPolicy: 'retain-original' }));
    assert.deepEqual(abilities('When this creature enters, draw a card. If this creature\'s spectacle cost was paid, instead draw three cards.', 'Creature — Devil'),
      [{ kind: 'triggered', trigger: { trigger: 'enters', what: { ref: 'this', noun: 'creature' } }, effects: [{ steps: [{ effect: 'self-replacement',
        original: { effect: 'draw', who: you, amount: one },
        condition: { condition: 'keyword-choice', keyword: 'spectacle', what: { ref: 'this', noun: 'creature' } },
        instead: [{ effect: 'draw', who: you, amount: n(3) }], targetPolicy: 'retain-original' }] }] }]);
  });
  it('raises the modal count (Go Nuts!, Pyrrhic Strike)', () => {
    const options = [
      { effects: [{ steps: [{ effect: 'draw', who: you, amount: one }] }] },
      { effects: [{ steps: [{ effect: 'gain-life', who: you, amount: n(2) }] }] },
    ];
    assert.deepEqual(abilities('Choose one. If this spell was cast using teamwork, choose both instead.\n• Draw a card.\n• You gain 2 life.'),
      spell({ effect: 'modal', count: { exactly: 1 }, options,
        conditionalCount: { condition: { condition: 'keyword-choice', keyword: 'teamwork', what: thisSpell }, count: { exactly: 2 } } }));
    assert.deepEqual(abilities('Choose one. If this spell\'s additional cost was paid, choose both instead.\n• Draw a card.\n• You gain 2 life.'),
      spell({ effect: 'modal', count: { exactly: 1 }, options,
        conditionalCount: { condition: { condition: 'additional-cost-paid', what: thisSpell }, count: { exactly: 2 } } }));
  });
  it('rejects new targets under a non-casting condition and unknown costs', () => {
    // A resolution-time condition can't change the announced targets.
    fails('Exile target creature with mana value 3 or less. If you control a Wizard, instead exile target creature and you gain 3 life.');
    // An "additional cost" belongs to a spell, not to a permanent.
    fails('Draw a card. If this creature\'s additional cost was paid, draw two cards instead.', 'Creature — Elf');
    // Not a keyword with a cost.
    fails('Draw a card. If this spell\'s flying cost was paid, draw two cards instead.');
    // A creature's sneak cost is asked about the permanent, never a target.
    fails('When this creature enters, exile target creature. If this creature\'s sneak cost was paid, instead exile target creature and target artifact.', 'Creature — Ninja');
  });
});
