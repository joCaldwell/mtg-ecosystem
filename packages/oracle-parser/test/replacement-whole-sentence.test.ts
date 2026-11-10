import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// Self-replacement of a whole sentence (CR 614.15), checked against the
// frozen Scryfall corpus: Careful Consideration, Primal Growth, Entish
// Restoration, Anticognition, Court of Ambition, Dazzling Denial, Overload,
// Stubborn Denial, Tallyman of Nurgle; alternative targets for a cost chosen
// while casting (601.2c, 702.174m): Long River's Pull, Wear Down.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed' as const, value });
const you = { player: 'you' };
const island = { condition: 'control', who: you, what: { ref: 'filter', filter: { subtypes: ['Island'] }, count: n(1) } };
const kicked = { condition: 'kicked', what: { ref: 'this', noun: 'spell' }, comparison: { op: 'ge', value: n(1) } };
const gift = { condition: 'keyword-choice', keyword: 'gift' };
const cards = (value: number) => ({ ref: 'filter', filter: { cls: 'card' }, count: n(value) });
const draw = (who: object, value: number) => ({ effect: 'draw', who, amount: n(value) });
const discard = (who: object, value: number) => ({ effect: 'discard', who, what: cards(value) });
const spellTarget = { ref: 'target', filter: { cls: 'spell' }, count: n(1) };
const thatSpell = { ref: 'that', filter: { cls: 'spell' } };
const pays = (symbol: string) => ({ condition: 'pays', who: { player: 'controller', of: { ref: 'it' } }, cost: [{ cost: 'mana', symbols: [symbol] }] });
function abilities(text: string, typeLine = 'Instant') {
  const parsed = parseOracleText(text, 'Test Card', { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities)) as unknown[];
}
const replaced = (node: object) => [{ kind: 'spell', effects: [{ steps: [{ effect: 'self-replacement', ...node }] }] }];
const fails = (text: string, typeLine = 'Instant') => assert.equal(parseOracleText(text, 'Test Card', { typeLine }).ok, false, text);

describe('self-replacement of a whole preceding sentence', () => {
  it('replaces every instruction of the sentence after a leading "instead" (Careful Consideration)', () => {
    const target = { player: 'target-player' }, same = { player: 'that-player' };
    assert.deepEqual(abilities('Target player draws four cards, then discards three cards. If you control an Island, instead that player draws four cards, then discards two cards.'), replaced({
      original: { sequence: [{ steps: [draw(target, 4), discard(target, 3)] }] }, condition: island,
      instead: [draw(same, 4), discard(same, 2)], targetPolicy: 'retain-original' }));
  });
  it('replaces only the directly preceding sentence (Entish Restoration)', () => {
    const [ability] = abilities('You gain 1 life. Draw a card, then discard a card. If you control an Island, instead draw two cards, then discard a card.') as { effects: unknown[] }[];
    assert.deepEqual(ability.effects, [
      { steps: [{ effect: 'gain-life', who: you, amount: n(1) }] },
      { steps: [{ effect: 'self-replacement', original: { sequence: [{ steps: [draw(you, 1), discard(you, 1)] }] }, condition: island,
        instead: [draw(you, 2), discard(you, 1)], targetPolicy: 'retain-original' }] }]);
  });
  it('moves an "unless" alternative into the replaced sentence (Stubborn Denial, Anticognition)', () => {
    const original = { sequence: [{ steps: [{ effect: 'counter', what: spellTarget }], condition: pays('1'), conditionKind: 'unless' }] };
    assert.deepEqual(abilities('Counter target spell unless its controller pays {1}. If you control an Island, counter that spell instead.'), replaced({
      original, condition: island, instead: [{ effect: 'counter', what: thatSpell }], targetPolicy: 'retain-original' }));
    assert.deepEqual(abilities('Counter target spell unless its controller pays {1}. If you control an Island, instead counter that spell, then scry 2.'), replaced({
      original, condition: island, instead: [{ effect: 'counter', what: thatSpell }, { effect: 'scry', amount: n(2) }], targetPolicy: 'retain-original' }));
  });
  it('keeps the replacement\'s own condition as a sentence (Dazzling Denial, Overload)', () => {
    assert.deepEqual(abilities('Counter target spell unless its controller pays {2}. If you control an Island, counter that spell unless its controller pays {4} instead.'), replaced({
      original: { sequence: [{ steps: [{ effect: 'counter', what: spellTarget }], condition: pays('2'), conditionKind: 'unless' }] }, condition: island,
      instead: { sequence: [{ steps: [{ effect: 'counter', what: thatSpell }], condition: pays('4'), conditionKind: 'unless' }] }, targetPolicy: 'retain-original' }));
    const manaValue = (value: number) => ({ condition: 'compare', value: { amount: 'attribute', of: { ref: 'it' }, attribute: 'mana-value' }, comparison: { op: 'le', value: n(value) } });
    assert.deepEqual(abilities('Counter target spell if its mana value is 2 or less. If this spell was kicked, counter that spell if its mana value is 4 or less instead.'), replaced({
      original: { sequence: [{ steps: [{ effect: 'counter', what: spellTarget }], condition: manaValue(2), conditionKind: 'if', conditionPosition: 'after' }] }, condition: kicked,
      instead: { sequence: [{ steps: [{ effect: 'counter', what: thatSpell }], condition: manaValue(4), conditionKind: 'if', conditionPosition: 'after' }] }, targetPolicy: 'retain-original' }));
  });
  it('keeps the single-instruction shape wherever it applied before', () => {
    assert.deepEqual(abilities('Draw a card. If you control an Island, instead draw two cards, then discard a card.'), replaced({
      original: draw(you, 1), condition: island, instead: [draw(you, 2), discard(you, 1)], targetPolicy: 'retain-original' }));
  });
  it('rejects a trailing "instead" after several instructions and a conditional original without a restated condition', () => {
    // Which of the two actions "draw two cards instead" replaces is not stated.
    fails('Draw a card, then discard a card. If you control an Island, draw two cards instead.');
    fails('You draw a card and you lose 1 life. If you control an Island, you draw two cards instead.');
    // Whether the replacement still depends on the first condition is not stated.
    fails('If you control a Swamp, draw a card. If you control an Island, instead draw two cards.');
    fails('You may pay {1}. If you do, draw a card. If you control an Island, draw two cards instead.');
    // Optional instructions and otherwise branches keep their unsupported scope.
    fails('You may draw a card, then discard a card. If you control an Island, instead draw two cards, then discard a card.');
    fails('If you control a Swamp, draw a card. Otherwise, discard a card. If you control an Island, instead draw two cards.');
  });
});

describe('alternative targets for a cost chosen while casting', () => {
  it('announces the replacement\'s targets only if the gift was promised (Long River\'s Pull, Wear Down)', () => {
    const creatureSpell = { ref: 'target', filter: { types: ['creature'], cls: 'spell' }, count: n(1) };
    assert.deepEqual(abilities('Counter target creature spell. If the gift was promised, instead counter target spell.'), replaced({
      original: { effect: 'counter', what: creatureSpell }, condition: gift, instead: [{ effect: 'counter', what: spellTarget }], targetPolicy: 'alternative-by-cost' }));
    const artifact = (count: number) => ({ ref: 'target', filter: { types: ['artifact'] }, count: n(count) });
    assert.deepEqual(abilities('Destroy target artifact. If this spell was bargained, instead destroy two target artifacts.', 'Sorcery'), replaced({
      original: { effect: 'destroy', what: artifact(1) }, condition: { condition: 'keyword-choice', keyword: 'bargain', what: { ref: 'this', noun: 'spell' } },
      instead: [{ effect: 'destroy', what: artifact(2) }], targetPolicy: 'alternative-by-cost' }));
  });
  it('keeps kicker\'s own policy and rejects new targets under a resolution-time condition', () => {
    assert.deepEqual(abilities('Counter target creature spell. If this spell was kicked, instead counter target spell.'), replaced({
      original: { effect: 'counter', what: { ref: 'target', filter: { types: ['creature'], cls: 'spell' }, count: n(1) } }, condition: kicked,
      instead: [{ effect: 'counter', what: spellTarget }], targetPolicy: 'alternative-by-kicker' }));
    fails('Counter target creature spell. If you control an Island, instead counter target spell.');
    // A permanent's triggered ability announces no targets by its spell's cost.
    fails('When this creature enters, destroy target artifact. If the gift was promised, instead destroy two target artifacts.', 'Creature — Raccoon');
  });
});
