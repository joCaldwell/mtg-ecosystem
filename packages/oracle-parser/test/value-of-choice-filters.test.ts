import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// CR 608.2d: a choice offered by an effect is announced while applying it.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function one(text: string, name?: string, typeLine?: string) {
  const result = parseOracleText(text, name, typeLine ? { typeLine } : undefined);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string, typeLine?: string) => assert.equal(parseOracleText(text, undefined, typeLine ? { typeLine } : undefined).ok, false, text);
const choice = (category: string) => ({ choice: 'fresh-choice', category, who: { player: 'you' }, when: 'instruction-applies' });

describe('"of the creature type / color of your choice" filters', () => {
  it('chooses one creature type for the instruction (Extinction)', () => {
    assert.deepEqual(one('Destroy all creatures of the creature type of your choice.', 'Extinction', 'Sorcery'), { kind: 'spell', effects: [{ steps: [{
      effect: 'destroy', what: { ref: 'all', filter: { types: ['creature'], valueOfChoice: choice('creature-type') } } }] }] });
  });
  it('chooses one color for the instruction (Wash Out)', () => {
    assert.deepEqual(one("Return all permanents of the color of your choice to their owners' hands.", 'Wash Out', 'Sorcery'), { kind: 'spell', effects: [{ steps: [{
      effect: 'return-to-hand', to: 'owner', what: { ref: 'all', filter: { cls: 'permanent', valueOfChoice: choice('color') } } }] }] });
  });
  it('keeps the linked-choice wording a different node', () => {
    assert.deepEqual(one('Destroy all creatures of the chosen type.', undefined, 'Sorcery'), { kind: 'spell', effects: [{ steps: [{
      effect: 'destroy', what: { ref: 'all', filter: { types: ['creature'], chosenType: { choice: 'chosen-type' } } } }] }] });
  });
  it('rejects target descriptions and static abilities, where no instruction is being applied', () => {
    fails('Return up to three target creature cards of the creature type of your choice from your graveyard to your hand.', 'Sorcery');
    fails("Return X target creatures of the creature type of your choice to their owner's hand.", 'Instant');
    fails('Creatures of the creature type of your choice get +1/+1.', 'Enchantment');
    fails('Creatures of the creature type of your choice have flying.', 'Enchantment');
    fails("Destroy all creatures of the creature type of an opponent's choice.", 'Sorcery');
  });
});
