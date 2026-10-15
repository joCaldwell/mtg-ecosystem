import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// Replacement of mana production and of losing unspent mana (CR 106.12b,
// 106.4, 500.5), checked against the frozen Scryfall corpus: Contamination,
// Ritual of Subdual, Pulse of Llanowar, Harvest Mage, Quarum Trench Gnomes,
// Mana Reflection, Nyxbloom Ancient, Pale Moon, Horizon Stone.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed' as const, value });
const you = { player: 'you' };
const land = { ref: 'filter', filter: { types: ['land'] }, count: n(1) };
const tapped = (what: object, extra: object = {}) => ({ event: 'tap-for-mana', what, ...extra });
function abilities(text: string, typeLine = 'Enchantment') {
  const parsed = parseOracleText(text, 'Test Card', { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities)) as unknown[];
}
const standing = (event: object, replace: object) => [{ kind: 'static', effect: { static: 'replacement', replacement: { event, occurrence: 'each', replace } } }];
const fails = (text: string, typeLine = 'Enchantment') => assert.equal(parseOracleText(text, 'Test Card', { typeLine }).ok, false, text);

describe('replacement of the mana a tapped permanent produces', () => {
  it('distinguishes replacing the type from replacing type and amount (Infernal Darkness / Contamination)', () => {
    assert.deepEqual(abilities('If a land is tapped for mana, it produces {B} instead of any other type.'),
      standing(tapped(land), { replacement: 'mana-type', type: { symbol: 'B' } }));
    assert.deepEqual(abilities('If a land is tapped for mana, it produces {B} instead of any other type and amount.'),
      standing(tapped(land), { replacement: 'mana', mana: { mana: 'fixed', symbols: ['B'] } }));
  });
  it('reads "colorless mana" as {C}, keeps a quantity test and a replaced color (Ritual of Subdual, Damping Sphere, Quarum Trench Gnomes)', () => {
    assert.deepEqual(abilities('If a land is tapped for mana, it produces colorless mana instead of any other type.'),
      standing(tapped(land), { replacement: 'mana-type', type: { symbol: 'C' } }));
    assert.deepEqual(abilities('If a land is tapped for two or more mana, it produces {C} instead of any other type and amount.', 'Artifact'),
      standing(tapped(land, { quantity: { op: 'ge', value: n(2) } }), { replacement: 'mana', mana: { mana: 'fixed', symbols: ['C'] } }));
    assert.deepEqual(abilities('{T}: If target Plains is tapped for mana, it produces colorless mana instead of white mana.', 'Creature — Gnome'), [{
      kind: 'activated', costs: [{ cost: 'tap-self' }], effects: [{ steps: [{ effect: 'create-replacement', replacement: {
        event: tapped({ ref: 'target', filter: { subtypes: ['Plains'] }, count: n(1) }), occurrence: 'each',
        replace: { replacement: 'mana-type', type: { symbol: 'C' }, only: 'white' } } }] }] }]);
  });
  it('keeps the tapping player and a color chosen as it applies (Pulse of Llanowar, Harvest Mage)', () => {
    assert.deepEqual(abilities('If a basic land you control is tapped for mana, it produces mana of a color of your choice instead of any other type.'),
      standing(tapped({ ref: 'filter', filter: { supertypes: ['basic'], types: ['land'], control: 'you' }, count: n(1) }),
        { replacement: 'mana-type', type: { choice: 'color', who: you } }));
    assert.deepEqual(abilities('Until end of turn, if you tap a land for mana, it produces one mana of a color of your choice instead of any other type and amount.', 'Instant'), [{
      kind: 'spell', effects: [{ steps: [{ effect: 'create-replacement', replacement: { event: tapped(land, { by: you }), occurrence: 'each',
        replace: { replacement: 'mana', mana: { mana: 'any-color', amount: n(1) } } }, duration: { duration: 'end-of-turn' } }] }] }]);
  });
  it('multiplies the mana without changing its types (Mana Reflection, Nyxbloom Ancient)', () => {
    const permanent = { ref: 'filter', filter: { cls: 'permanent' }, count: n(1) };
    assert.deepEqual(abilities('If you tap a permanent for mana, it produces twice as much of that mana instead.'),
      standing(tapped(permanent, { by: you }), { replacement: 'modify-quantity', change: { operation: 'multiply', factor: 2 } }));
    assert.deepEqual(abilities('If you tap a permanent for mana, it produces three times as much of that mana instead.'),
      standing(tapped(permanent, { by: you }), { replacement: 'modify-quantity', change: { operation: 'multiply', factor: 3 } }));
  });
  it('rejects unstated amounts, unknown symbols and other subjects', () => {
    fails('If a land is tapped for mana, it produces colorless mana instead of any other type and amount.');
    fails('If a land is tapped for mana, it produces one mana of a color of your choice instead of any other type.');
    fails('If a land is tapped for mana, it produces one mana of a color of your choice instead of any other type and amount.');
    fails('If a land is tapped for mana, it produces {2} instead of any other type.');
    fails('If a land is tapped for mana, that player produces {B} instead of any other type.');
    fails('If a land is tapped for mana, it produces {B} instead.');
    fails('If a land is tapped for mana, it produces twice as much mana instead.');
  });
});

describe('replacement of losing unspent mana', () => {
  it('keeps the mana as mana of the printed type (Horizon Stone, Omnath, Locus of All)', () => {
    const lose = { event: 'lose-unspent-mana', who: you };
    assert.deepEqual(abilities('If you would lose unspent mana, that mana becomes colorless instead.', 'Artifact'), standing(lose, { replacement: 'keep-mana', as: { symbol: 'C' } }));
    assert.deepEqual(abilities('If you would lose unspent mana, that mana becomes black instead.', 'Creature — Elemental'), standing(lose, { replacement: 'keep-mana', as: { symbol: 'B' } }));
  });
  it('rejects other outcomes for the lost mana', () => {
    fails('If you would lose unspent mana, that mana becomes colorless.', 'Artifact');
    fails('If you would lose unspent mana, draw a card instead.', 'Artifact');
  });
});
