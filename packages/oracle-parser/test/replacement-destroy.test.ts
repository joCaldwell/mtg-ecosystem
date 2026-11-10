import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// Replacement of destruction (CR 701.8a, 614.8, 701.19b), checked against
// the frozen Scryfall corpus: Mossbridge Troll, Clergy of the Holy Nimbus,
// Crackling Emergence, Harmonious Emergence.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const affected = { ref: 'replacement-event-object', role: 'affected' };
function abilities(text: string, typeLine = 'Creature — Troll') {
  const parsed = parseOracleText(text, 'Test Card', { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities)) as unknown[];
}
const standing = (replacement: object) => [{ kind: 'static', effect: { static: 'replacement', replacement } }];
const fails = (text: string, typeLine = 'Creature — Troll') => assert.equal(parseOracleText(text, 'Test Card', { typeLine }).ok, false, text);

describe('replacing destruction', () => {
  it('reads static regeneration with its implicit "instead" (Mossbridge Troll)', () => {
    assert.deepEqual(abilities('If this creature would be destroyed, regenerate it.'), standing({ event: { event: 'destroy', what: { ref: 'this', noun: 'creature' } }, occurrence: 'each',
      replace: { replacement: 'effects', effects: [{ steps: [{ effect: 'regenerate', what: affected }] }] } }));
  });
  it('binds "that land" to the permanent that would be destroyed and keeps the printed duration (Crackling Emergence)', () => {
    assert.deepEqual(abilities('If enchanted land would be destroyed, instead sacrifice this Aura and that land gains indestructible until end of turn.', 'Enchantment — Aura'),
      standing({ event: { event: 'destroy', what: { ref: 'enchanted', noun: 'land' } }, occurrence: 'each', replace: { replacement: 'effects', effects: [{ steps: [
        { effect: 'sacrifice', who: { player: 'you' }, what: { ref: 'this', noun: 'aura' } },
        { effect: 'gain-abilities', what: affected, abilities: [{ gained: 'keyword', keyword: { keyword: 'indestructible' } }], duration: { duration: 'end-of-turn' } }] }] } }));
  });
  it('is not dying, and needs "instead" for anything but regeneration', () => {
    assert.notDeepEqual(abilities('If this creature would die, exile it instead.'), abilities('If this creature would be destroyed, exile it instead.'));
    fails('If this creature would be destroyed, exile it.');
    fails('If this creature would die, regenerate it.');
    fails('If this creature would be destroyed, regenerate it. Draw a card.');
    fails('If enchanted land would be destroyed, instead sacrifice this Aura and that creature gains indestructible until end of turn.', 'Enchantment — Aura');
  });
});
