import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// CR 608.2d: a choice made as the effect is created; 611.2: characteristic
// changing effects. https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function one(text: string, name?: string, typeLine?: string) {
  const result = parseOracleText(text, name, typeLine ? { typeLine } : undefined);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string) => assert.equal(parseOracleText(text, undefined, { typeLine: 'Creature — Shapeshifter' }).ok, false, text);
const n = (value: number) => ({ amount: 'fixed', value });
const it_ = { ref: 'it' };
const creature = (p: number, t: number, keyword: string) => ({ effect: 'modify-characteristics', what: it_, snapshot: 'effect-created', modifiers: [
  { modifier: 'types', change: { cardTypes: { operation: 'set', types: ['creature'] } } },
  { modifier: 'base-pt', power: n(p), toughness: n(t) },
  { modifier: 'abilities', abilities: [{ gained: 'keyword', keyword: { keyword } }] }] });

describe('becomes your choice of', () => {
  it('chooses one complete characteristic set (Corrupted Shapeshifter)', () => {
    assert.deepEqual(one('As this creature enters, it becomes your choice of a 3/3 creature with flying, a 2/5 creature with vigilance, or a 0/12 creature with defender.',
      undefined, 'Creature — Shapeshifter'), { kind: 'static', effect: { static: 'as-enters', what: { ref: 'this', noun: 'creature' }, effects: [{ steps: [{
        effect: 'choice', who: { player: 'you' }, options: [creature(3, 3, 'flying'), creature(2, 5, 'vigilance'), creature(0, 12, 'defender')] }] }] } });
  });
  it('chooses between bare power/toughness pairs (Aquamorph Entity)', () => {
    assert.deepEqual(one('As this creature enters, it becomes your choice of 5/1 or 1/5.', undefined, 'Creature — Shapeshifter').effect.effects, [{ steps: [{
      effect: 'choice', who: { player: 'you' }, options: [{ effect: 'become', what: it_, spec: { power: n(5), toughness: n(1) } },
        { effect: 'become', what: it_, spec: { power: n(1), toughness: n(5) } }] }] }]);
  });
  it('differs from a single description', () => {
    assert.deepEqual(one('As this creature enters, it becomes a 3/3 creature with flying.', undefined, 'Creature — Shapeshifter').effect.effects, [{ steps: [creature(3, 3, 'flying')] }]);
  });
  it('rejects unfinished lists and an ambiguous trailing type rider', () => {
    fails('As this creature enters, it becomes your choice of a 3/3 creature.');
    fails('As this creature enters, it becomes your choice of a 3/3 creature, a 2/2 creature with flying.');
    fails('As this creature enters, it becomes your choice of a 3/3 artifact creature or a 1/6 Wall artifact creature with defender in addition to its other types.');
    fails('As this creature enters, it becomes your choice of a 3/3 creature or flying.');
  });
});
