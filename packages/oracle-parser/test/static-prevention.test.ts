import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// Static damage prevention scoped to your turn (CR 615.1, 611.3a). Checked
// against the frozen Scryfall corpus: Personal Sanctuary, Guardian Naga,
// Gideon Blackblade.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function abilities(text: string, typeLine = 'Enchantment') {
  const parsed = parseOracleText(text, 'Test Card', { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities)) as unknown[];
}
const fails = (text: string, typeLine = 'Enchantment') => assert.equal(parseOracleText(text, 'Test Card', { typeLine }).ok, false, text);
const preventAllTo = (to: object) => ({ static: 'prevent-damage', prevention: { eventScope: { sources: 'simultaneous', recipients: 'simultaneous' },
  shield: { shield: 'events', occurrence: 'each', prevent: { prevent: 'all' } }, kind: 'all', to: [to] } });
const yourTurn = (effect: object) => [{ kind: 'static', effect: { static: 'conditional', condition: { condition: 'your-turn' }, effect } }];

describe('static prevention during your turn', () => {
  it('reads a leading "During your turn," (Personal Sanctuary, Guardian Naga)', () => {
    assert.deepEqual(abilities('During your turn, prevent all damage that would be dealt to you.'), yourTurn(preventAllTo({ player: 'you' })));
    assert.deepEqual(abilities('During your turn, prevent all damage that would be dealt to this creature.', 'Creature — Snake'),
      yourTurn(preventAllTo({ ref: 'this', noun: 'creature' })));
  });
  it('reads a trailing "during your turn" the same way (Gideon Blackblade)', () => {
    assert.deepEqual(abilities('Prevent all damage that would be dealt to this creature during your turn.', 'Creature — Snake'),
      yourTurn(preventAllTo({ ref: 'this', noun: 'creature' })));
  });
  it('rejects other turn periods', () => {
    fails('Prevent all damage that would be dealt to you during your next turn.');
    fails('Prevent all damage that would be dealt to you during combat.');
  });
});
