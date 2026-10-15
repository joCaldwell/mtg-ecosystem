import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// Replacement of the amount of damage a source would deal (CR 614.1a,
// 701.10g, 107.1a), checked against the frozen Scryfall corpus: Gisela, Blade
// of Goldnight, Divine Presence, Benevolent Unicorn, Ghosts of the Innocent,
// Equal Treatment, Isengard Unleashed, Overblaze, Raphael, the Muscle.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed' as const, value });
const anySource = { ref: 'filter', filter: {}, count: n(1) };
const permanentOrPlayer = [{ ref: 'filter', filter: { cls: 'permanent' }, count: n(1) }, { player: 'a-player' }];
const opposing = [{ player: 'an-opponent' }, { ref: 'filter', filter: { cls: 'permanent', control: 'opponent' }, count: n(1) }];
const multiply = (factor: number) => ({ replacement: 'modify-quantity', change: { operation: 'multiply', factor } });
function abilities(text: string, typeLine = 'Enchantment') {
  const parsed = parseOracleText(text, 'Test Card', { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities)) as unknown[];
}
const standing = (event: object, replace: object) => [{ kind: 'static', effect: { static: 'replacement', replacement: { event, occurrence: 'each', replace } } }];
const fails = (text: string, typeLine = 'Enchantment') => assert.equal(parseOracleText(text, 'Test Card', { typeLine }).ok, false, text);

describe('replacement of the amount of damage a source would deal', () => {
  it('accepts "that source" and either order of the repeated recipients (Gisela, Blade of Goldnight)', () => {
    const doubled = standing({ event: 'damage', kind: 'all', source: anySource, to: opposing }, multiply(2));
    assert.deepEqual(abilities('If a source would deal damage to an opponent or a permanent an opponent controls, that source deals double that damage to that player or permanent instead.'), doubled);
    assert.deepEqual(abilities('If a source would deal damage to an opponent or a permanent an opponent controls, it deals double that damage to that permanent or player instead.'), doubled);
  });
  it('keeps a threshold on the watched damage and sets the new amount (Divine Presence, Equal Treatment)', () => {
    assert.deepEqual(abilities('If a source would deal 4 or more damage to a permanent or player, that source deals 3 damage to that permanent or player instead.'),
      standing({ event: 'damage', kind: 'all', source: anySource, quantity: { op: 'ge', value: n(4) }, to: permanentOrPlayer }, { replacement: 'modify-quantity', change: { operation: 'set', amount: n(3) } }));
    assert.deepEqual(abilities('If a source would deal exactly 2 damage to a permanent or player, it deals 3 damage instead.'),
      standing({ event: 'damage', kind: 'all', source: anySource, quantity: { op: 'eq', value: n(2) }, to: permanentOrPlayer }, { replacement: 'modify-quantity', change: { operation: 'set', amount: n(3) } }));
    assert.deepEqual(abilities('If any source would deal 1 or more damage to a permanent or player this turn, it deals 2 damage to that permanent or player instead.', 'Instant'), [{ kind: 'spell', effects: [{ steps: [{
      effect: 'create-replacement', replacement: { event: { event: 'damage', kind: 'all', source: anySource, quantity: { op: 'ge', value: n(1) }, to: permanentOrPlayer },
        occurrence: 'each', replace: { replacement: 'modify-quantity', change: { operation: 'set', amount: n(2) } } }, duration: { duration: 'this-turn' } }] }] }]);
  });
  it('subtracts from and halves the damage (Benevolent Unicorn, Ghosts of the Innocent)', () => {
    assert.deepEqual(abilities('If a spell would deal damage to a permanent or player, it deals that much damage minus 1 to that permanent or player instead.'),
      standing({ event: 'damage', kind: 'all', source: { ref: 'filter', filter: { cls: 'spell' }, count: n(1) }, to: permanentOrPlayer }, { replacement: 'modify-quantity', change: { operation: 'subtract', amount: n(1) } }));
    assert.deepEqual(abilities('If a source would deal damage to a permanent or player, it deals half that damage, rounded down, to that permanent or player instead.'),
      standing({ event: 'damage', kind: 'all', source: anySource, to: permanentOrPlayer }, { replacement: 'modify-quantity', change: { operation: 'half', round: 'down' } }));
  });
  it('reads recipients printed after the duration, and "each time" as each occurrence (Isengard Unleashed, Overblaze)', () => {
    const tripled = { effect: 'create-replacement', replacement: { event: { event: 'damage', kind: 'all', source: { ref: 'filter', filter: { control: 'you' }, count: n(1) }, to: opposing },
      occurrence: 'each', replace: multiply(3) }, duration: { duration: 'this-turn' } };
    assert.deepEqual(abilities('If a source you control would deal damage this turn to an opponent or a permanent an opponent controls, it deals triple that damage instead.', 'Sorcery'), [{ kind: 'spell', effects: [{ steps: [tripled] }] }]);
    assert.deepEqual(abilities('If a source you control would deal damage to an opponent or a permanent an opponent controls this turn, it deals triple that damage instead.', 'Sorcery'), [{ kind: 'spell', effects: [{ steps: [tripled] }] }]);
    assert.deepEqual(abilities('Each time target permanent would deal damage to a permanent or player this turn, it deals double that damage to that permanent or player instead.', 'Instant'), [{ kind: 'spell', effects: [{ steps: [{
      effect: 'create-replacement', replacement: { event: { event: 'damage', kind: 'all', source: { ref: 'target', filter: { cls: 'permanent' }, count: n(1) }, to: permanentOrPlayer },
        occurrence: 'each', replace: multiply(2) }, duration: { duration: 'this-turn' } }] }] }]);
  });
  it('rejects unstated rounding, other sources and other recipients', () => {
    fails('If a source would deal damage to a permanent or player, it deals half that damage to that permanent or player instead.');
    fails('If a source would deal damage to a permanent or player, it deals half that damage, rounded down, to you instead.');
    fails('If a source would deal damage to a creature you control, that source deals double that damage to that player or permanent instead.');
    fails('If a source would deal damage to a permanent or player, that creature deals double that damage to that permanent or player instead.');
    fails('If a source would deal exactly 2 or more damage to a permanent or player, it deals 3 damage instead.');
    fails('If a source would deal damage to a permanent or player, it deals that much damage minus 1 plus 2 instead.');
  });
});

describe('"Double all damage X would deal" (CR 701.10g)', () => {
  it('is the same replacement as "it deals double that damage instead"', () => {
    assert.deepEqual(abilities('Double all damage ~ would deal.', 'Creature — Berserker'), standing({ event: 'damage', kind: 'all', source: { ref: 'self' } }, multiply(2)));
    assert.deepEqual(abilities('Double all damage ~ would deal.', 'Creature — Berserker'), abilities('If ~ would deal damage, it deals double that damage instead.', 'Creature — Berserker'));
    assert.deepEqual(abilities('Double all damage equipped creature would deal.', 'Artifact — Equipment'),
      standing({ event: 'damage', kind: 'all', source: { ref: 'equipped', noun: 'creature' } }, multiply(2)));
  });
  it('matches each source of a plural subject (Raphael, the Muscle; Absorbing Man and Titania)', () => {
    assert.deepEqual(abilities('Double all damage that creatures you control with counters on them would deal.', 'Creature — Mutant'), standing({ event: 'damage', kind: 'all',
      source: { ref: 'filter', filter: { types: ['creature'], control: 'you', counterConstraints: [{ counter: 'any', comparison: { op: 'ge', value: n(1) } }] }, count: n(1) } }, multiply(2)));
    assert.deepEqual(abilities('Double all damage that creature sources you control would deal.', 'Creature — Villain'), standing({ event: 'damage', kind: 'all',
      source: { ref: 'filter', filter: { types: ['creature'], cls: 'source', control: 'you' }, count: n(1) } }, multiply(2)));
  });
  it('rejects targets, durations and recipients it does not state', () => {
    fails('Double all damage target creature would deal.', 'Instant');
    fails('Double all damage ~ would deal this turn.', 'Creature — Berserker');
    fails('Double all damage ~ would deal to creatures.', 'Creature — Berserker');
    fails('Double all damage that would be dealt.', 'Enchantment');
  });
});
