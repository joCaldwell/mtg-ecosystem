import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// CR 201.4/201.4a (card-name choices), 205.3e (subtype choices).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function one(text: string, name?: string) {
  const result = parseOracleText(text, name);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string, name?: string) => assert.equal(parseOracleText(text, name).ok, false, text);
const you = { player: 'you' };
const choose = (choice: object) => ({ kind: 'spell', effects: [{ steps: [{ effect: 'choose-value', who: you, ...choice }] }] });

describe('described card-name choices', () => {
  it('restricts the chosen name to cards matching the description', () => {
    assert.deepEqual(one('Choose a nonland card name.'), choose({ category: 'card-name', filter: { nonTypes: ['land'], cls: 'card' } }));
    assert.deepEqual(one('Choose a creature card name.'), choose({ category: 'card-name', filter: { types: ['creature'], cls: 'card' } }));
    assert.deepEqual(one('Choose a noncreature, nonland card name.'),
      choose({ category: 'card-name', filter: { nonTypes: ['creature', 'land'], cls: 'card' } }));
    assert.deepEqual(one('Choose a nonbasic land card name.'),
      choose({ category: 'card-name', filter: { nonSupertypes: ['basic'], types: ['land'], cls: 'card' } }));
  });
  it('keeps an exclusion separate from a restriction', () => {
    assert.deepEqual(one('Choose a card name other than a basic land card name.'),
      choose({ category: 'card-name', excluding: { supertypes: ['basic'], types: ['land'], cls: 'card' } }));
  });
  it('treats "any card name" as the unrestricted choice and keeps the chooser (Sorcerous Spyglass)', () => {
    assert.deepEqual(one('Choose any card name.'), choose({ category: 'card-name' }));
    assert.deepEqual(one('Choose a card name.'), choose({ category: 'card-name' }));
    assert.deepEqual(one('As this creature enters, choose a nonland card name.'), {
      kind: 'static', effect: { static: 'as-enters', what: { ref: 'this', noun: 'creature' },
        effects: [{ steps: [{ effect: 'choose-value', who: you, category: 'card-name', filter: { nonTypes: ['land'], cls: 'card' } }] }] },
    });
  });
  it('still selects a card, not a name, without the word "name"', () => {
    assert.deepEqual(one('Choose a creature card.'), { kind: 'spell', effects: [{ steps: [{ effect: 'choose-object', who: you,
      what: { ref: 'filter', filter: { types: ['creature'], cls: 'card' }, count: { amount: 'fixed', value: 1 } } }] }] });
  });
  it('rejects compound or malformed name choices', () => {
    fails('Choose a nonland card name and a number greater than 0.');
    fails('Choose a card name other than a card name.');
    fails('Choose a card name other than basic.');
    fails('Choose two nonland card names.');
  });
});

describe('creature-type choices with an excluded type', () => {
  it('records the excluded type and links "that type" to the choice (Imagecrafter)', () => {
    assert.deepEqual(one('{T}: Choose a creature type other than Wall. Target creature becomes that type until end of turn.'), {
      kind: 'activated', costs: [{ cost: 'tap-self' }], effects: [
        { steps: [{ effect: 'choose-value', who: you, category: 'creature-type', excluding: ['Wall'] }] },
        { steps: [{ effect: 'modify-characteristics', what: { ref: 'target', filter: { types: ['creature'] }, count: { amount: 'fixed', value: 1 } },
          modifiers: [{ modifier: 'types', change: { chosenSubtype: { operation: 'set', choice: { choice: 'chosen-type', kind: 'creature' } } } }],
          snapshot: 'effect-created', duration: { duration: 'end-of-turn' } }] },
      ],
    });
  });
  it('rejects plural or non-creature exclusions', () => {
    fails('Choose a creature type other than Walls.');
    fails('Choose a creature type other than Forest.');
    fails('Choose a creature type other than artifact.');
    fails('Choose a basic land type other than Forest.');
  });
});
