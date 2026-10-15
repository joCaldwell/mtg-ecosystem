import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// A reflexive ability created by a standing replacement's own instruction
// ("exile it instead. When you do, …", CR 603.12, 614.1). Checked against the
// frozen Scryfall corpus: Head of the Hunt, Nemata, Primeval Warden,
// Valentin, Dean of the Vein.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed' as const, value });
function abilities(text: string, typeLine = 'Creature — Elf') {
  const parsed = parseOracleText(text, 'Test Card', { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities)) as unknown[];
}
const fails = (text: string, typeLine = 'Creature — Elf') => assert.equal(parseOracleText(text, 'Test Card', { typeLine }).ok, false, text);

describe('reflexive trigger after a replacement', () => {
  it('follows the replacement instruction (Head of the Hunt)', () => {
    assert.deepEqual(abilities('If a creature an opponent controls would die, exile it instead. When you do, create a 2/2 green Wolf creature token.'),
      [{ kind: 'static', effect: { static: 'replacement', replacement: {
        event: { event: 'move-zone', what: { ref: 'filter', filter: { types: ['creature'], control: 'opponent' }, count: n(1) },
          from: { zone: 'battlefield' }, to: { zone: 'graveyard' } },
        occurrence: 'each', replace: { replacement: 'effects', effects: [
          { steps: [{ effect: 'exile', what: { ref: 'replacement-event-object', role: 'affected' } }] },
          { steps: [{ effect: 'create-reflexive-trigger', event: { event: 'preceding-action', who: { player: 'you' }, performed: true },
            effects: [{ steps: [{ effect: 'create-token', count: n(1), token: { colors: ['green'], supertypes: [], types: ['creature'],
              subtypes: ['Wolf'], keywords: [], power: n(2), toughness: n(2) } }] }] }] },
        ] } } } }]);
  });
  it('rejects reflexive bodies that refer into the replaced event or a different outcome', () => {
    // "it" would have to be the exiled card, which this scope doesn't bind.
    fails('If a creature an opponent controls would die, exile it instead. When you do, put a +1/+1 counter on it.');
    fails('If a creature an opponent controls would die, exile it instead. When you don\'t, create a 2/2 green Wolf creature token.');
    fails('If a creature an opponent controls would die, exile it instead. When you do, create a 2/2 green Wolf creature token. Draw a card.');
  });
});
