import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// "The same is true for K1, K2, and K3": the first sentence is repeated once
// per listed keyword, with its keyword replaced everywhere.
function one(text: string, name?: string) {
  const result = parseOracleText(text, name);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string, name?: string) => assert.equal(parseOracleText(text, name).ok, false, text);
const fixed = (value: number) => ({ amount: 'fixed', value });
const self = { ref: 'this', noun: 'creature' };

describe('"The same is true for" keyword lists', () => {
  it('turns a static template into a group of per-keyword static effects', () => {
    const modify = (keyword: string) => ({ static: 'conditional',
      condition: { condition: 'control', who: { player: 'an-opponent' }, what: { ref: 'filter', filter: { types: ['creature'], withAbility: [keyword] }, count: fixed(1) } },
      effect: { static: 'modify', what: self, modifiers: [{ modifier: 'abilities', abilities: [{ gained: 'keyword', keyword: { keyword } }] }] } });
    const actual = one('As long as an opponent controls a creature with flying, this creature has flying. The same is true for first strike and trample.');
    assert.deepEqual(actual, { kind: 'static', effect: { static: 'group', effects: [modify('flying'), modify('first strike'), modify('trample')] } });
  });
  it('appends per-keyword sentences to a triggered template (Odric, Lunarch Marshal)', () => {
    const template = 'At the beginning of each combat, creatures you control gain first strike until end of turn if a creature you control has first strike.';
    const actual = one(`${template} The same is true for flying, deathtouch, and vigilance.`);
    const sentences = (keyword: string) => one(template.replaceAll('first strike', keyword)).effects;
    assert.deepEqual(actual, { ...one(template), effects: [...sentences('first strike'), ...sentences('flying'), ...sentences('deathtouch'), ...sentences('vigilance')] });
    assert.equal(actual.effects.length, 4);
    assert.deepEqual(actual.effects[1].steps[0].abilities, [{ gained: 'keyword', keyword: { keyword: 'flying' } }]);
    assert.deepEqual(actual.effects[1].condition.keyword, 'flying');
  });
  it('rejects keyword families, repeated or missing template keywords, and non-keyword items', () => {
    fails('As long as a card exiled with this creature has flying, this creature has flying. The same is true for first strike, landwalk, and trample.');
    fails('As long as a card exiled with this creature has flying, this creature has flying. The same is true for first strike, protection, and trample.');
    fails('As long as a card exiled with this creature has flying, this creature has flying. The same is true for first strike, flying, and trample.');
    fails('This creature has flying. The same is true for first strike and trample.');
    fails('As long as a card exiled with this creature has flying, this creature has flying. The same is true for first strike and ward {2}.');
    fails('As long as a card exiled with this creature has flying, this creature has flying. The same is true for first strike and blue creatures.');
  });
});
