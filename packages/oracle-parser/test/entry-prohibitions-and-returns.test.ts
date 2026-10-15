import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// Oracle wordings checked against the frozen corpus; rules from the
// Comprehensive Rules of 2026-09-25.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function abilities(text: string, typeLine = 'Sorcery') {
  const r = parseOracleText(text, undefined, { typeLine });
  assert.ok(r.ok, JSON.stringify(r.lines));
  return JSON.parse(JSON.stringify(r.abilities));
}
const fails = (text: string, typeLine = 'Sorcery') => assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);

describe('cards that can\'t enter the battlefield (CR 101.2)', () => {
  const prohibit = (filter: object) => [{ kind: 'static', effect: { static: 'prohibit',
    prohibition: { prohibition: 'enter-battlefield', what: { ref: 'each', filter } } } }];

  it('states the described cards, with one or several zones', () => {
    assert.deepEqual(abilities('Creature cards in graveyards can\'t enter the battlefield.', 'Artifact'),
      prohibit({ types: ['creature'], cls: 'card', zone: { zone: 'graveyard' } }));
    assert.deepEqual(abilities('Creature cards in graveyards and libraries can\'t enter the battlefield.', 'Artifact'),
      prohibit({ types: ['creature'], cls: 'card', anyOf: [{ zone: { zone: 'graveyard' } }, { zone: { zone: 'library' } }] }));
  });

  it('rejects near misses', () => {
    fails('Creature cards in graveyards can\'t enter.', 'Artifact');
    fails('Creature cards in graveyards and libraries and hands can\'t enter the battlefield.', 'Artifact');
    fails('Creature cards in graveyards and libraries can\'t enter the battlefield tapped.', 'Artifact');
  });
});

describe('return with the destination stated first (CR 400.3)', () => {
  const ret = (what: object) => [{ kind: 'triggered', trigger: { trigger: 'enters', what: { ref: 'self' } },
    effects: [{ steps: [{ effect: 'return-to-hand', what, to: 'your' }] }] }];
  it('is the same move as "return … to your hand"', () => {
    const fronted = abilities('When ~ enters, return to your hand all creature cards in your graveyard.', 'Creature — Human');
    assert.deepEqual(fronted, ret({ ref: 'all', filter: { types: ['creature'], cls: 'card', zone: { zone: 'graveyard', owner: 'your' } } }));
    assert.deepEqual(abilities('When ~ enters, return all creature cards in your graveyard to your hand.', 'Creature — Human'), fronted);
  });
  it('rejects a second destination', () => {
    fails('When ~ enters, return to your hand all creature cards in your graveyard to your hand.', 'Creature — Human');
  });
});

