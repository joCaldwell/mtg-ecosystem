import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// "Excess damage is dealt to that creature's controller instead" modifies
// the damage event it follows (CR 120.4a). Checked against the frozen
// Scryfall corpus: Flame Spill, Pigment Storm, Gandalf's Sanction,
// Ravenous Tyrannosaurus.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed' as const, value });
function abilities(text: string, typeLine = 'Sorcery') {
  const parsed = parseOracleText(text, 'Test Card', { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities)) as unknown[];
}
const fails = (text: string, typeLine = 'Sorcery') => assert.equal(parseOracleText(text, 'Test Card', { typeLine }).ok, false, text);
const controller = { player: 'controller', of: { ref: 'that', filter: { types: ['creature'] } } };

describe('excess damage redirection', () => {
  it('modifies the preceding damage instruction (Flame Spill)', () => {
    assert.deepEqual(abilities('~ deals 4 damage to target creature. Excess damage is dealt to that creature\'s controller instead.'),
      [{ kind: 'spell', effects: [{ steps: [{ effect: 'damage', source: { ref: 'self' }, amount: n(4),
        to: { to: 'object', ref: { ref: 'target', filter: { types: ['creature'] }, count: n(1) } }, excessTo: controller }] }] }]);
  });
  it('keeps the definition of X on the sentence (Gandalf\'s Sanction)', () => {
    const [ability] = abilities('~ deals X damage to target creature, where X is the number of cards in your hand. Excess damage is dealt to that creature\'s controller instead.') as
      { effects: { steps: { excessTo?: unknown }[]; bindings?: unknown }[] }[];
    assert.equal(ability.effects.length, 1);
    assert.ok(ability.effects[0].bindings);
    assert.deepEqual(ability.effects[0].steps[0].excessTo, controller);
  });
  it('rejects excess damage without one damaged permanent', () => {
    fails('~ deals 4 damage to each creature. Excess damage is dealt to that creature\'s controller instead.');
    fails('~ deals 4 damage to target player. Excess damage is dealt to that player\'s controller instead.');
    fails('Draw a card. Excess damage is dealt to that creature\'s controller instead.');
    fails('~ deals 4 damage to target creature. If you control a Wizard, excess damage is dealt to that creature\'s controller instead.');
    fails('~ deals 2 damage to target creature and 2 damage to another target creature. Excess damage is dealt to that creature\'s controller instead.');
  });
});
