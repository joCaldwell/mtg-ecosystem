import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// CR 113.3a: only instants and sorceries have resolving spell abilities.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe('face card types constrain top-level ability classification', () => {
  it('does not discard the rules-bearing Case solved gate as a flavor word', () => {
    const result = parseOracleText('Solved — Whenever you cast a spell, draw a card.');
    assert.ok(result.ok);
    assert.deepEqual(result.abilities, [{ kind: 'case-solved', ability: {
      kind: 'triggered', abilityWord: undefined, condition: undefined,
      trigger: { trigger: 'cast', who: { player: 'you' }, what: { cls: 'spell' } },
      effects: [{ steps: [{ effect: 'draw', who: { player: 'you' }, amount: { amount: 'fixed', value: 1 } }] }],
    } }]);
    assert.equal(parseOracleText('Solved — Draw a card.').ok, false);
  });
  it('does not silently reinterpret permanent text as one-shot spell instructions', () => {
    for (const typeLine of ['Creature — Wizard', 'Enchantment — Aura', 'Artifact', 'Land', 'Battle — Siege', 'Legendary Planeswalker — Jace']) {
      const result = parseOracleText('Draw a card.', undefined, { typeLine });
      assert.equal(result.ok, false, typeLine);
      assert.equal(result.lines[0].ok, false);
      assert.match(!result.lines[0].ok ? result.lines[0].error : '', /unsupported permanent ability/);
    }
  });
  it('accepts a complete instant or sorcery face and separately classified permanent abilities', () => {
    for (const typeLine of ['Instant', 'Sorcery', 'Kindred Instant — Wizard']) {
      const result = parseOracleText('Draw a card.', undefined, { typeLine });
      assert.ok(result.ok);
      assert.deepEqual(result.abilities, [{ kind: 'spell', abilityWord: undefined, effects: [{ steps: [{ effect: 'draw', who: { player: 'you' }, amount: { amount: 'fixed', value: 1 } }] }] }]);
    }
    const triggered = parseOracleText('When this creature enters, draw a card.', undefined, { typeLine: 'Creature — Wizard' });
    assert.ok(triggered.ok);
    assert.equal(triggered.abilities[0].kind, 'triggered');
    const quoted = parseOracleText('Creatures you control have "{T}: Draw a card."', undefined, { typeLine: 'Enchantment' });
    assert.ok(quoted.ok);
    assert.equal(quoted.abilities[0].kind, 'static');
  });
});
