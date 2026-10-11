import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// Actual full card sources checked in the frozen Oracle corpus. CR400.7,
// 508.4,601.2f: count declared attacker incarnations, not surviving objects;
// reduction changes the payment, not the printed mana cost.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const count = { amount: 'combat-history-count', action: 'attacked', during: 'this-turn', distinct: 'object-incarnations' };
const cost = { kind: 'static', effect: { static: 'cost-modification', scope: { action: 'cast', spells: { ref: 'this', noun: 'spell' } },
  change: { direction: 'reduce', symbols: ['1'] }, multiplier: count } };
function abilities(text: string, name: string, typeLine: string) {
  const result = parseOracleText(text, name, { typeLine, includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}

describe('historical attacker counts', () => {
  it('retains every detail of the three affected cost-reduction cards', () => {
    const first = 'This spell costs {1} less to cast for each creature that attacked this turn.\n';
    assert.deepEqual(abilities(first + 'Draw three cards.', 'Rowdy Research', 'Instant'), [cost,
      { kind: 'spell', effects: [{ steps: [{ effect: 'draw', who: { player: 'you' }, amount: { amount: 'fixed', value: 3 } }] }] },
    ]);
    assert.deepEqual(abilities(first + 'Menace (This creature can\'t be blocked except by two or more creatures.)', 'The Mary Janes', 'Legendary Creature — Human Bard Performer'), [cost,
      { kind: 'keywords', keywords: [{ keyword: 'menace' }] },
    ]);
    assert.deepEqual(abilities(first + 'Witchstalker Frenzy deals 5 damage to target creature.', 'Witchstalker Frenzy', 'Instant'), [cost,
      { kind: 'spell', effects: [{ steps: [{ effect: 'damage', source: { ref: 'self' }, amount: { amount: 'fixed', value: 5 }, to: {
        to: 'object', ref: { ref: 'target', filter: { types: ['creature'] }, count: { amount: 'fixed', value: 1 }, selectionId: 'line:1:5' },
      } }] }] },
    ]);
  });
  it('uses historical cardinality for a quantity, and rejects unknown current/history qualifier mixtures', () => {
    assert.deepEqual(abilities('Draw a card for each creature that attacked this turn.', '', 'Sorcery'), [{ kind: 'spell', effects: [{ steps: [{
      effect: 'draw', who: { player: 'you' }, amount: { amount: 'product', factors: [{ amount: 'fixed', value: 1 }, count] },
    }] }] }]);
    for (const text of ['This spell costs {1} less to cast for each creature you control that attacked this turn.',
      'Draw a card for each creature you control that attacked this turn.',
      'Draw X cards, where X is the number of creatures you control that attacked this turn.'])
      assert.equal(parseOracleText(text).ok, false, text);
  });
});
