import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// "If a spell cast this way would be put into a graveyard, exile it instead"
// (CR 614.1a, 608.2c), checked against the frozen Scryfall corpus: Bilbo,
// Thief in the Night; Scholar of the Lost Trove; Radiant Scrollwielder;
// Flotsam // Jetsam; Kess, Dissident Mage; Jaya Ballard; Sins of the Past;
// Halo Forager; Power Pack; Red Sun's Zenith. The "that spell" wording was
// already accepted (Impulsivity, Mission Briefing) and keeps its node.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed' as const, value });
const you = { player: 'you' };
const instantOrSorcery = { allOf: [{ anyOf: [{ types: ['instant'] }, { types: ['sorcery'] }] }] };
const castThisWay = { relation: 'event-result', event: 'cast', scope: 'this-way' };
const exileIt = { replacement: 'effects', effects: [{ steps: [{ effect: 'exile', what: { ref: 'replacement-event-object', role: 'affected' } }] }] };
const watch = (what: object, owner: string) => ({ event: { event: 'move-zone', what, to: { zone: 'graveyard', owner } }, occurrence: 'each', replace: exileIt });
const spellCastThisWay = (filter: object = {}) => ({ ref: 'filter', filter: { ...filter, cls: 'spell', relation: castThisWay }, count: n(1) });
function abilities(text: string, typeLine = 'Sorcery') {
  const parsed = parseOracleText(text, 'Test Card', { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities)) as unknown[];
}
const fails = (text: string, typeLine = 'Sorcery') => assert.equal(parseOracleText(text, 'Test Card', { typeLine }).ok, false, text);

describe('replacement watching spells cast this way', () => {
  it('follows a one-shot cast instruction and keeps the printed spell filter (Scholar of the Lost Trove)', () => {
    const card = { ref: 'target', filter: { allOf: [{ anyOf: [{ types: ['instant'] }, { types: ['sorcery'] }, { types: ['artifact'] }] }], cls: 'card' }, count: n(1) };
    assert.deepEqual(abilities('You may cast target instant, sorcery, or artifact card from your graveyard without paying its mana cost. If an instant or sorcery spell cast this way would be put into your graveyard, exile it instead.'), [{
      kind: 'spell', effects: [
        { steps: [{ effect: 'play-card', instruction: { who: you, action: 'cast', what: card, from: { zone: 'graveyard', owner: 'your' }, alternativeCost: { cost: 'without-mana-cost' } }, optional: true }] },
        { steps: [{ effect: 'create-replacement', replacement: watch(spellCastThisWay(instantOrSorcery), 'your') }] }] }]);
  });
  it('follows a lasting cast permission and distinguishes "a graveyard" (Urabrask // The Great Work)', () => {
    const [ability] = abilities('Until end of turn, you may cast instant and sorcery spells from any graveyard. If a spell cast this way would be put into a graveyard, exile it instead.') as { effects: { steps: unknown[] }[] }[];
    assert.deepEqual(ability.effects[1], { steps: [{ effect: 'create-replacement', replacement: watch(spellCastThisWay(), 'any') }] });
  });
  it('is part of the same static ability as the permission (Kess, Dissident Mage)', () => {
    assert.deepEqual(abilities('Once during each of your turns, you may cast an instant or sorcery spell from your graveyard. If a spell cast this way would be put into your graveyard, exile it instead.', 'Creature — Human Wizard'), [{
      kind: 'static', effect: { static: 'group', effects: [
        { static: 'play-permission', permission: { who: you, action: 'cast', what: { ref: 'filter', filter: { ...instantOrSorcery, cls: 'spell' }, count: n(1) },
          from: { zone: 'graveyard', owner: 'your' }, useLimit: { maximum: 1, period: 'turn', during: 'your' } } },
        { static: 'replacement', replacement: watch(spellCastThisWay(), 'your') }] } }]);
  });
  it('allows a later instruction about the source itself (Sins of the Past)', () => {
    const [ability] = abilities('Until end of turn, you may cast target instant card from your graveyard. If that spell would be put into your graveyard, exile it instead. Exile Test Card.') as { effects: unknown[] }[];
    assert.deepEqual(ability.effects.slice(1), [
      { steps: [{ effect: 'create-replacement', replacement: watch({ ref: 'that', filter: { cls: 'spell' } }, 'your') }] },
      { steps: [{ effect: 'exile', what: { ref: 'self' } }] }]);
  });
  it('stays inside a reflexive trigger with the cast it watches (Halo Forager)', () => {
    const [ability] = abilities('When this creature enters, you may pay {1}. When you do, you may cast target instant card from a graveyard. If that spell would be put into a graveyard, exile it instead.', 'Creature — Faerie') as
      { effects: { steps: { effect: string; effects: { steps: { effect: string }[] }[] }[] }[] }[];
    const reflexive = ability.effects[1].steps[0];
    assert.equal(reflexive.effect, 'create-reflexive-trigger');
    assert.deepEqual(reflexive.effects.map(sentence => sentence.steps[0].effect), ['play-card', 'create-replacement']);
    assert.deepEqual(reflexive.effects[1], { steps: [{ effect: 'create-replacement', replacement: watch({ ref: 'that', filter: { cls: 'spell' } }, 'any') }] });
  });
  it('rejects the rider without its cast permission and keeps linked continuations rejected', () => {
    // A standing rule needs the permission that "this way" refers to.
    fails('If a spell cast this way would be put into your graveyard, exile it instead.', 'Enchantment');
    fails('Flying\nIf a spell cast this way would be put into your graveyard, exile it instead.', 'Creature — Bird');
    // A static permission accepts only the cast-this-way replacement after it.
    fails('Once during each of your turns, you may cast an instant spell from your graveyard. If a card would be put into your graveyard, exile it instead.', 'Enchantment');
    // A following sentence about the replaced object is not an independent instruction.
    fails('If a creature would die this turn, exile it instead. Return it to the battlefield at the beginning of the next end step.');
    // "When you do" after the replacement is a reflexive trigger of the replacing action (Valentin, Nemata).
    const replaced = parseOracleText('If a creature would die this turn, exile it instead. When you do, draw a card.', 'Fixture', { typeLine: 'Sorcery' });
    assert.equal(replaced.ok, true);
    assert.match(JSON.stringify(replaced.abilities), /"replace":\{"replacement":"effects","effects":\[\{"steps":\[\{"effect":"exile".*"create-reflexive-trigger"/);
    // Reflexive bodies accept no other second sentence.
    fails('When this creature enters, you may pay {1}. When you do, you may cast target instant card from a graveyard. If a creature would die this turn, exile it instead.', 'Creature — Faerie');
  });
});
