import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// CR 201.2a/201.4 (names), 607.2d (linked choices), 608.2c (later text of
// the same instruction refers to its earlier choice).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function one(text: string, name?: string) {
  const result = parseOracleText(text, name);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string, name?: string) => assert.equal(parseOracleText(text, name).ok, false, text);
const n = (value: number) => ({ amount: 'fixed', value });
const you = { player: 'you' };
const chosenName = { choice: 'chosen-value', category: 'card-name', which: 'chosen' };
const chosenColor = { choice: 'chosen-value', category: 'color', which: 'chosen' };
const chosenType = { choice: 'chosen-type' };

describe('objects with the chosen name', () => {
  it('filters by a linked name choice (Meddling Mage, Declaration of Naught)', () => {
    assert.deepEqual(one("Spells with the chosen name can't be cast."), { kind: 'static', effect: { static: 'prohibit',
      prohibition: { prohibition: 'cast', who: { player: 'each-player' }, what: { ref: 'each', filter: { cls: 'spell', chosenName } } } } });
    assert.deepEqual(one('{U}: Counter target spell with the chosen name.'), { kind: 'activated', costs: [{ cost: 'mana', symbols: ['U'] }],
      effects: [{ steps: [{ effect: 'counter', what: { ref: 'target', filter: { cls: 'spell', chosenName }, count: n(1) } }] }] });
  });
  it('reads "that name" as the name chosen earlier in the same body (Cabal Therapy)', () => {
    const hand = { ref: 'all', filter: { cls: 'card', zone: { zone: 'hand', owner: 'their' } } };
    const target = { player: 'target-player' };
    const body = (wording: string) => one(`Choose a nonland card name. Target player reveals their hand and discards all cards with ${wording}.`);
    const expected = { kind: 'spell', effects: [
      { steps: [{ effect: 'choose-value', who: you, category: 'card-name', filter: { nonTypes: ['land'], cls: 'card' } }] },
      { steps: [{ effect: 'reveal', who: target, what: hand }, { effect: 'discard', who: target, what: { ref: 'all', filter: { cls: 'card', chosenName } } }] },
    ] };
    assert.deepEqual(body('that name'), expected);
    assert.deepEqual(body('the chosen name'), expected);
  });
  it('tests a referenced card against the chosen name (Cursed Scroll)', () => {
    const ability = one('{3}, {T}: Choose a card name, then reveal a card at random from your hand. If that card has the chosen name, this artifact deals 2 damage to any target.');
    assert.deepEqual(ability.effects[1], { steps: [{ effect: 'damage', source: { ref: 'this', noun: 'artifact' }, amount: n(2), to: { to: 'any-target' } }],
      condition: { condition: 'is-filter', what: { ref: 'that', filter: { cls: 'card' } }, filter: { chosenName } }, conditionKind: 'if' });
  });
  it('rejects "that name" without exactly one card-name choice in the body', () => {
    // Deadly Cover-Up: the name of the exiled card, not a chosen name.
    fails("Exile a card from an opponent's graveyard. Then search its owner's graveyard, hand, and library for any number of cards with that name and exile them.");
    fails('Exile all cards with that name from your graveyard.');
    fails('Choose a creature type. Exile all cards with that name from your graveyard.');
    fails('Choose a card name. Choose a card name. Exile all cards with that name from your graveyard.');
  });
});

describe('"of that color" and "of that type" after a choice in the same body', () => {
  it('uses the chosen-color reference (Persecute)', () => {
    const target = { player: 'target-player' };
    assert.deepEqual(one('Choose a color. Target player reveals their hand and discards all cards of that color.'), { kind: 'spell', effects: [
      { steps: [{ effect: 'choose-value', who: you, category: 'color' }] },
      { steps: [{ effect: 'reveal', who: target, what: { ref: 'all', filter: { cls: 'card', zone: { zone: 'hand', owner: 'their' } } } },
        { effect: 'discard', who: target, what: { ref: 'all', filter: { cls: 'card', chosenColor } } }] },
    ] });
  });
  it('uses the same chosen-type reference as "of the chosen type" (Outbreak)', () => {
    const body = (wording: string) => one(`Choose a creature type. All creatures of ${wording} get -1/-1 until end of turn.`);
    const minus = { sign: -1, amount: n(1) };
    const expected = { kind: 'spell', effects: [
      { steps: [{ effect: 'choose-value', who: you, category: 'creature-type' }] },
      { steps: [{ effect: 'pump', what: { ref: 'all', filter: { types: ['creature'], chosenType } }, power: minus, toughness: minus,
        duration: { duration: 'end-of-turn' } }] },
    ] };
    assert.deepEqual(body('that type'), expected);
    assert.deepEqual(body('the chosen type'), expected);
  });
  it('rejects the pronoun forms without one matching choice', () => {
    // Enchantment Alteration: the type of the enchanted permanent, not a choice.
    fails('Attach target Aura attached to a creature or land to another permanent of that type.');
    fails('Destroy all creatures of that color.');
    fails('Choose a color. Destroy all creatures of that type.');
    fails('Choose a creature type. Destroy all creatures of that color.');
    fails('Choose two colors. Destroy all creatures of that color.');
    fails('Choose a color. Choose a color. Destroy all creatures of that color.');
    fails('Choose Island or Swamp. Destroy all lands of that type.');
  });
});
