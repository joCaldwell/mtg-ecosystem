import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// Replacement of entering the battlefield (CR 614.12a, 614.13, 400.7d),
// checked against the frozen Scryfall corpus: Lotus Vale, Heart of Yavimaya,
// Mox Diamond, Containment Priest, Hallowed Moonlight, Mistcaller.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed' as const, value });
const you = { player: 'you' };
const affected = { ref: 'replacement-event-object', role: 'affected' };
const enters = (what: object) => ({ event: 'move-zone', what, to: { zone: 'battlefield' } });
const outcome = (action: object) => ({ replacement: 'effects', effects: [
  { steps: [action] },
  { condition: { condition: 'you-do' }, conditionKind: 'if', steps: [{ effect: 'move-zone', what: affected, to: { zone: 'battlefield' } }] },
  { condition: { condition: 'you-dont' }, conditionKind: 'if', steps: [{ effect: 'move-zone', what: affected, to: { zone: 'graveyard', owner: 'its-owner' } }] }] });
function abilities(text: string, typeLine = 'Land') {
  const parsed = parseOracleText(text, 'Test Card', { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities)) as unknown[];
}
const fails = (text: string, typeLine = 'Land') => assert.equal(parseOracleText(text, 'Test Card', { typeLine }).ok, false, text);

describe('replacing a permanent\'s own entry with a payment', () => {
  it('keeps both outcomes inside the replacement (Lotus Vale)', () => {
    assert.deepEqual(abilities('If this land would enter, sacrifice two untapped lands instead. If you do, put this land onto the battlefield. If you don\'t, put it into its owner\'s graveyard.'), [{
      kind: 'static', effect: { static: 'replacement', replacement: { event: enters({ ref: 'this', noun: 'land' }), occurrence: 'each',
        replace: outcome({ effect: 'sacrifice', who: you, what: { ref: 'filter', filter: { status: ['untapped'], types: ['land'] }, count: n(2) } }) } } }]);
  });
  it('makes the action optional, not the replacement (Mox Diamond)', () => {
    assert.deepEqual(abilities('If this artifact would enter, you may discard a land card instead. If you do, put this artifact onto the battlefield. If you don\'t, put it into its owner\'s graveyard.', 'Artifact'), [{
      kind: 'static', effect: { static: 'replacement', replacement: { event: enters({ ref: 'this', noun: 'artifact' }), occurrence: 'each',
        replace: outcome({ effect: 'discard', who: you, what: { ref: 'filter', filter: { types: ['land'], cls: 'card' }, count: n(1) }, optional: true }) } } }]);
  });
  it('rejects incomplete or mismatched outcomes', () => {
    fails('If this land would enter, sacrifice a Forest instead. If you do, put this land onto the battlefield.');
    fails('If this land would enter, sacrifice a Forest instead. If you do, put this land onto the battlefield. If you don\'t, put it into your hand.');
    fails('If this land would enter, sacrifice a Forest instead. If you do, put this creature onto the battlefield. If you don\'t, put it into its owner\'s graveyard.');
    fails('If this land would enter, draw a card instead. If you do, put this land onto the battlefield. If you don\'t, put it into its owner\'s graveyard.');
    // The outcome pair belongs only to the permanent's own entry.
    fails('If a land would enter, sacrifice a Forest instead. If you do, put this land onto the battlefield. If you don\'t, put it into its owner\'s graveyard.', 'Enchantment');
  });
});

describe('replacing the entry of permanents that weren\'t cast', () => {
  const exile = { replacement: 'effects', effects: [{ steps: [{ effect: 'exile', what: affected }] }] };
  const notCast = { condition: 'not', of: { condition: 'was-cast', what: affected } };
  it('tests the entering permanent\'s own casting (Containment Priest, Hallowed Moonlight)', () => {
    const creature = { ref: 'filter', filter: { nonToken: true, types: ['creature'] }, count: n(1) };
    assert.deepEqual(abilities('If a nontoken creature would enter and it wasn\'t cast, exile it instead.', 'Creature — Human Cleric'), [{ kind: 'static', effect: {
      static: 'replacement', replacement: { event: enters(creature), occurrence: 'each', replace: exile, condition: notCast } } }]);
    assert.deepEqual(abilities('Until end of turn, if a creature would enter and it wasn\'t cast, exile it instead.', 'Instant'), [{ kind: 'spell', effects: [{ steps: [{
      effect: 'create-replacement', replacement: { event: enters({ ref: 'filter', filter: { types: ['creature'] }, count: n(1) }), occurrence: 'each', replace: exile, condition: notCast },
      duration: { duration: 'end-of-turn' } }] }] }]);
  });
  it('reads "would enter the battlefield" and "would be put onto the battlefield" as the same event', () => {
    const short = abilities('If a creature would enter, exile it instead.', 'Enchantment');
    assert.deepEqual(short, abilities('If a creature would enter the battlefield, exile it instead.', 'Enchantment'));
    assert.deepEqual(short, abilities('If a creature would be put onto the battlefield, exile it instead.', 'Enchantment'));
  });
  it('rejects casting conditions it cannot represent', () => {
    fails('If a creature would enter and it wasn\'t cast or no mana was spent to cast it, exile it instead.', 'Enchantment');
    fails('If a creature would die and it wasn\'t cast, exile it instead.', 'Enchantment');
  });
});
