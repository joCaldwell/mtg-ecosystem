import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// Replacement of counters being put on listed recipients (CR 614.1a, 122.1,
// 110.1), checked against the frozen Scryfall corpus: Winding Constrictor,
// Mauhúr, Uruk-hai Captain, Loading Zone, Lae'zel, Vlaakith's Champion.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed' as const, value });
const oneOrMore = { op: 'ge', value: n(1) };
const listed = (...alternatives: object[]) => ({ ref: 'filter', filter: { allOf: [{ anyOf: alternatives }], control: 'you' }, count: n(1) });
function abilities(text: string, typeLine = 'Enchantment') {
  const parsed = parseOracleText(text, 'Test Card', { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities)) as unknown[];
}
const standing = (event: object, change: object) => [{ kind: 'static', effect: { static: 'replacement', replacement: { event, occurrence: 'each', replace: { replacement: 'modify-quantity', change } } } }];
const fails = (text: string, typeLine = 'Enchantment') => assert.equal(parseOracleText(text, 'Test Card', { typeLine }).ok, false, text);

describe('counter replacements on a list of permanent kinds', () => {
  it('reads "that permanent" as the recipient among controlled permanent kinds (Winding Constrictor)', () => {
    assert.deepEqual(abilities('If one or more counters would be put on an artifact or creature you control, that many plus one of each of those kinds of counters are put on that permanent instead.', 'Creature — Snake'),
      standing({ event: 'put-counters', on: [listed({ types: ['artifact'] }, { types: ['creature'] })], counter: 'any', cause: 'any', quantity: oneOrMore }, { operation: 'add', amount: n(1) }));
  });
  it('keeps a comma-separated list inside the event (Mauhúr, Uruk-hai Captain; Loading Zone)', () => {
    assert.deepEqual(abilities('If one or more +1/+1 counters would be put on an Army, Goblin, or Orc you control, that many plus one +1/+1 counters are put on it instead.', 'Creature — Orc'),
      standing({ event: 'put-counters', on: [listed({ subtypes: ['Army'] }, { subtypes: ['Goblin'] }, { subtypes: ['Orc'] })], counter: { type: '+1/+1' }, cause: 'any', quantity: oneOrMore }, { operation: 'add', amount: n(1) }));
    assert.deepEqual(abilities('If one or more counters would be put on a creature, Spacecraft, or Planet you control, twice that many of each of those kinds of counters are put on it instead.'),
      standing({ event: 'put-counters', on: [listed({ types: ['creature'] }, { subtypes: ['Spacecraft'] }, { subtypes: ['Planet'] })], counter: 'any', cause: 'any', quantity: oneOrMore }, { operation: 'multiply', factor: 2 }));
  });
  it('adds the acting player as an alternative recipient (Lae\'zel, Vlaakith\'s Champion)', () => {
    assert.deepEqual(abilities('If you would put one or more counters on a creature or planeswalker you control or on yourself, put that many plus one of each of those kinds of counters on that permanent or player instead.', 'Creature — Gith'),
      standing({ event: 'put-counters', by: { player: 'you' }, on: [listed({ types: ['creature'] }, { types: ['planeswalker'] }), { player: 'you' }], counter: 'any', cause: 'any', quantity: oneOrMore }, { operation: 'add', amount: n(1) }));
  });
  it('rejects "that permanent" for uncontrolled or non-permanent recipients, and a mismatched repeat', () => {
    fails('If one or more counters would be put on an artifact or creature, that many plus one of each of those kinds of counters are put on that permanent instead.');
    fails('If one or more counters would be put on an artifact or creature you control, that many plus one of each of those kinds of counters are put on that creature instead.');
    fails('If you would put one or more counters on a creature you control or on yourself, put that many plus one of each of those kinds of counters on that permanent instead.');
    fails('If one or more counters would be put on an Army, Goblin, or Orc you control, draw a card, then put them on it instead.');
  });
});
