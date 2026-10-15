import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

function one(text: string, name?: string, typeLine?: string) {
  const result = parseOracleText(text, name, typeLine ? { typeLine } : {});
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string, name?: string) => assert.equal(parseOracleText(text, name).ok, false, text);
const fixed = (value: number) => ({ amount: 'fixed', value });

describe('"<color> is the most common color among …"', () => {
  it('keeps the color, the counted group and the tie policy (Goham Djinn)', () => {
    assert.deepEqual(one('This creature gets -2/-2 as long as black is the most common color among all permanents or is tied for most common.'), {
      kind: 'static', effect: { static: 'modify', what: { ref: 'this', noun: 'creature' },
        modifiers: [{ modifier: 'pt', power: { sign: -1, amount: fixed(2) }, toughness: { sign: -1, amount: fixed(2) } }],
        condition: { condition: 'most-common-color', color: 'black', among: { ref: 'all', filter: { cls: 'permanent' } }, ties: 'included' } } });
  });
  it('distinguishes an excluded tie', () => {
    assert.deepEqual(one('This creature gets +1/+1 as long as white is the most common color among all permanents but isn\'t tied for most common.').effect.condition,
      { condition: 'most-common-color', color: 'white', among: { ref: 'all', filter: { cls: 'permanent' } }, ties: 'excluded' });
  });
  it('rejects a missing tie policy and a single referent', () => {
    fails('This creature gets -2/-2 as long as black is the most common color among all permanents.');
    fails('This creature gets -2/-2 as long as black is the most common color among target permanent or is tied for most common.');
  });
});

describe('"[it] becomes plotted / foretold / saddled"', () => {
  it('gives an exiled card the plotted designation (Aven Interrupter)', () => {
    assert.deepEqual(one('When this creature enters, exile target spell. It becomes plotted.', undefined, 'Creature — Bird Rogue'), {
      kind: 'triggered', trigger: { trigger: 'enters', what: { ref: 'this', noun: 'creature' } },
      effects: [{ steps: [{ effect: 'exile', what: { ref: 'target', filter: { cls: 'spell' }, count: fixed(1) } }] },
        { steps: [{ effect: 'become-status', what: { ref: 'it' }, status: 'plotted' }] }] });
  });
  it('reads foretold the same way, with no foretell cost (The Foretold Soldier)', () => {
    assert.deepEqual(one('Whenever this creature deals damage, exile it face down. It becomes foretold.', undefined, 'Creature — Alien Zombie Soldier').effects, [
      { steps: [{ effect: 'exile', what: { ref: 'it' }, faceDown: true }] },
      { steps: [{ effect: 'become-status', what: { ref: 'it' }, status: 'foretold' }] }]);
  });
  it('accepts saddled with its own end-of-turn lifetime only (Kolodin, Triumph Caster)', () => {
    assert.deepEqual(one('Whenever a Mount you control enters, it becomes saddled until end of turn.', undefined, 'Legendary Creature — Human Pilot').effects, [
      { steps: [{ effect: 'become-status', what: { ref: 'it' }, status: 'saddled' }] }]);
    fails('Whenever a Mount you control enters, it becomes saddled until your next turn.');
    fails('Whenever a Mount you control enters, it becomes plotted until end of turn.');
  });
});

describe('attachment conditions "… is / are attached to …"', () => {
  const aCreature = (filter: object) => ({ ref: 'filter', filter, count: fixed(1) });
  it('tests a definite subject itself', () => {
    assert.deepEqual(one('As long as this Equipment is attached to a creature, you don\'t lose the game for having 0 or less life.', undefined, 'Artifact — Equipment').effect.condition,
      { condition: 'is-filter', what: { ref: 'this', noun: 'equipment' }, filter: { attachedTo: aCreature({ types: ['creature'] }) } });
  });
  it('counts numeric and indefinite subjects (Brass Knuckles, Face of Divinity)', () => {
    assert.deepEqual(one('Equipped creature has double strike as long as two or more Equipment are attached to it.', undefined, 'Artifact — Equipment').effect.condition,
      { condition: 'count', of: { subtypes: ['Equipment'], attachedTo: { ref: 'it' } }, comparison: { op: 'ge', value: fixed(2) } });
    assert.deepEqual(one('As long as another Aura is attached to enchanted creature, it has first strike and lifelink.', undefined, 'Enchantment — Aura').effect.condition,
      { condition: 'count', of: { other: true, subtypes: ['Aura'], attachedTo: { ref: 'enchanted', noun: 'creature' } }, comparison: { op: 'ge', value: fixed(1) } });
    assert.deepEqual(one('Equipped creature has first strike as long as an Equipment is attached to a creature you control.', undefined, 'Artifact — Equipment').effect.condition,
      { condition: 'count', of: { subtypes: ['Equipment'], attachedTo: aCreature({ types: ['creature'], control: 'you' }) }, comparison: { op: 'ge', value: fixed(1) } });
  });
  it('rejects a universal operand and an unrelated predicate', () => {
    fails('Equipped creature has double strike as long as two or more Equipment are attached to each creature you control.');
    fails('Equipped creature has double strike as long as this Equipment is attached to all creatures.');
  });
});

describe('"X is …" as a separate sentence of an activated ability', () => {
  it('defines the X of the whole ability, cost included (Voodoo Doll)', () => {
    assert.deepEqual(one('{X}{X}, {T}: This artifact deals damage equal to the number of pin counters on it to any target. X is the number of pin counters on this artifact.', undefined, 'Artifact'), {
      kind: 'activated', costs: [{ cost: 'mana', symbols: ['X', 'X'] }, { cost: 'tap-self' }],
      effects: [{ steps: [{ effect: 'damage', source: { ref: 'this', noun: 'artifact' },
        amount: { amount: 'counter-count', counter: { type: 'pin' }, on: { ref: 'it' } }, to: { to: 'any-target' } }] }],
      variableDefinitions: [{ variable: 'x', value: { amount: 'counter-count', counter: { type: 'pin' }, on: { ref: 'this', noun: 'artifact' } } }] });
  });
  it('keeps the referenced object symbolic (Prototype Portal)', () => {
    assert.deepEqual(one('{X}, {T}: Draw a card. X is the mana value of that card.', undefined, 'Artifact').variableDefinitions,
      [{ variable: 'x', value: { amount: 'attribute', of: { ref: 'that', filter: { cls: 'card' } }, attribute: 'mana-value' } }]);
  });
  it('rejects a definition without an X, a constant, a competing where-X and non-activated lines', () => {
    fails('{T}: Draw a card. X is the number of cards in your hand.');
    fails('{X}, {T}: Draw a card. X is 3.');
    fails('{X}, {T}: Draw X cards, where X is the number of cards in your hand. X is the number of creatures you control.');
    fails('Draw X cards. X is the number of cards in your hand.');
    fails('{X}, {T}: Draw a card. X is the number of cards in your hand. Activate only as a sorcery.');
  });
});

describe('base power / toughness filters and amounts (CR 208.4b)', () => {
  const base = (property: string, op: string, value: number) => ({ property, comparison: { op, value: fixed(value) } });
  it('reads "with base power and toughness N/N" as both base values (Duskana)', () => {
    assert.deepEqual(one('When this creature enters, draw a card for each creature you control with base power and toughness 2/2.', undefined, 'Creature — Bear').effects, [{ steps: [{
      effect: 'draw', who: { player: 'you' }, amount: fixed(1),
      forEach: { types: ['creature'], control: 'you', properties: [base('base-power', 'eq', 2), base('base-toughness', 'eq', 2)] } }] }]);
  });
  it('reads "base power or toughness" as either base value, and a single base value', () => {
    assert.deepEqual(one('Equipped creature gets +1/+1 for each creature you control with base power or toughness 1 or less.', undefined, 'Artifact — Equipment').effect.modifiers[0].forEach,
      { types: ['creature'], control: 'you', anyOf: [{ properties: [base('base-power', 'le', 1)] }, { properties: [base('base-toughness', 'le', 1)] }] });
    assert.deepEqual(one('Draw a card for each creature you control with base toughness 3 or greater.').effects[0].steps[0].forEach,
      { types: ['creature'], control: 'you', properties: [base('base-toughness', 'ge', 3)] });
  });
  it('reads a possessive base power as its own attribute (Curie)', () => {
    assert.deepEqual(one('Whenever this creature deals combat damage to a player, draw cards equal to its base power.', undefined, 'Artifact Creature — Robot').effects[0].steps[0].amount,
      { amount: 'attribute', of: { ref: 'it' }, attribute: 'base-power' });
  });
  it('rejects malformed base stat phrases', () => {
    fails('Draw a card for each creature you control with base power and toughness 2.');
    fails('Draw a card for each creature you control with base 2/2.');
  });
});

describe('"that isn\'t enchanted" negated statuses', () => {
  it('excludes the status (Winds of Rath, Brainspoil)', () => {
    assert.deepEqual(one('Destroy all creatures that aren\'t enchanted. They can\'t be regenerated.'), { kind: 'spell', effects: [{ steps: [{
      effect: 'destroy', what: { ref: 'all', filter: { types: ['creature'], nonStatus: ['enchanted'] } }, regenerationForbidden: true }] }] });
    assert.deepEqual(one('Return target permanent that isn\'t enchanted to its owner\'s hand.').effects[0].steps[0].what,
      { ref: 'target', filter: { cls: 'permanent', nonStatus: ['enchanted'] }, count: fixed(1) });
  });
  it('leaves statuses with an antonym word to their positive form', () => {
    fails('Destroy target creature that isn\'t tapped.');
    fails('Destroy target creature that isn\'t enchanted by an Aura you control.');
  });
});

describe('"is enchanted by N Auras" and base P/T with granted abilities', () => {
  const auras = (op: string, value: number) => ({ condition: 'count', of: { subtypes: ['Aura'], attachedTo: { ref: 'this', noun: 'creature' } }, comparison: { op, value: fixed(value) } });
  it('counts the Auras attached to the subject (Timber Paladin)', () => {
    assert.deepEqual(one('As long as this creature is enchanted by exactly two Auras, it has base power and toughness 5/5 and vigilance.', undefined, 'Artifact Creature — Knight'), {
      kind: 'static', effect: { static: 'conditional', condition: auras('eq', 2), effect: { static: 'modify', what: { ref: 'it' }, modifiers: [
        { modifier: 'base-pt', power: fixed(5), toughness: fixed(5) },
        { modifier: 'abilities', abilities: [{ gained: 'keyword', keyword: { keyword: 'vigilance' } }] }] } } });
    assert.deepEqual(one('As long as this creature is enchanted by three or more Auras, it has base power and toughness 10/10, vigilance, and trample.').effect.effect.modifiers[1],
      { modifier: 'abilities', abilities: [{ gained: 'keyword', keyword: { keyword: 'vigilance' } }, { gained: 'keyword', keyword: { keyword: 'trample' } }] });
  });
  it('requires Auras and a definite subject', () => {
    fails('As long as this creature is enchanted by exactly two Equipment, it has base power and toughness 5/5.');
    fails('As long as a creature is enchanted by exactly two Auras, this creature has base power and toughness 5/5.');
  });
});
