import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// Additional effects of a prevention effect (CR 615.5): a prevented-damage
// condition limited to the damage's source, a triggered ability created by
// each prevention ("When damage is prevented this way", 603.12), and a
// per-1-damage exile quantity. Checked against the frozen Scryfall corpus:
// Shadowbane, Samite Ministration, Phyrexian Vindicator, Immortal Coil.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const you = { player: 'you' };
const prevented = { amount: 'prevention-event-quantity', quantity: 'prevented' };
const thisCreature = { ref: 'this', noun: 'creature' };
function abilities(text: string, typeLine = 'Instant') {
  const parsed = parseOracleText(text, 'Test Card', { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities)) as unknown[];
}
const fails = (text: string, typeLine = 'Instant') => assert.equal(parseOracleText(text, 'Test Card', { typeLine }).ok, false, text);
const staticPrevention = (to: object, after: object[]) => [{ kind: 'static', effect: { static: 'prevent-damage', prevention: {
  shield: { shield: 'events', occurrence: 'each', prevent: { prevent: 'all' } }, kind: 'all',
  eventScope: { sources: 'simultaneous', recipients: 'each' }, to: [to], after } } }];
const spellPrevention = (after: object[]) => [{ kind: 'spell', effects: [{ steps: [{ effect: 'prevent-damage', prevention: {
  eventScope: { sources: 'simultaneous', recipients: 'simultaneous' },
  shield: { shield: 'events', occurrence: 'each', prevent: { prevent: 'all' } }, kind: 'all', to: [you], after },
  duration: { duration: 'this-turn' } }] }] }];

describe('prevention riders', () => {
  it('limits the prevented-damage condition to a source (Shadowbane)', () => {
    assert.deepEqual(abilities('Prevent all damage that would be dealt to you this turn. If damage from a black source is prevented this way, you gain that much life.'),
      spellPrevention([{ steps: [{ effect: 'gain-life', who: you, amount: prevented }],
        condition: { condition: 'damage-prevented', source: { colors: ['black'], cls: 'source' } }, conditionKind: 'if' }]));
  });
  it('creates a triggered ability for each prevention (Phyrexian Vindicator, Samite Ministration)', () => {
    assert.deepEqual(abilities('If damage would be dealt to this creature, prevent that damage. When damage is prevented this way, this creature deals that much damage to any other target.', 'Creature — Phyrexian Horror'),
      staticPrevention(thisCreature, [{ steps: [{ effect: 'create-reflexive-trigger', event: { event: 'damage-prevented' }, effects: [{ steps: [{
        effect: 'damage', source: thisCreature, amount: prevented, to: { to: 'object', ref: { ref: 'any-target', other: true } } }] }] }] }]));
    assert.deepEqual(abilities('Prevent all damage that would be dealt to you this turn. Whenever damage from a red source is prevented this way this turn, you gain that much life.'),
      spellPrevention([{ steps: [{ effect: 'create-reflexive-trigger', event: { event: 'damage-prevented', source: { colors: ['red'], cls: 'source' } },
        effects: [{ steps: [{ effect: 'gain-life', who: you, amount: prevented }] }] }] }]));
  });
  it('exiles one card from your graveyard for each 1 damage prevented (Immortal Coil)', () => {
    assert.deepEqual(abilities('If damage would be dealt to you, prevent that damage. Exile a card from your graveyard for each 1 damage prevented this way.', 'Artifact'),
      staticPrevention(you, [{ steps: [{ effect: 'exile', what: { ref: 'filter', filter: { cls: 'card', zone: { zone: 'graveyard', owner: 'your' } }, count: prevented } }] }]));
  });
  it('divides a prevention budget among targets (Embolden, Pollen Remedy)', () => {
    const divided = (value: number) => ({ effect: 'prevent-damage', prevention: {
      eventScope: { sources: 'simultaneous', recipients: 'each' },
      shield: { shield: 'damage-budget', amount: { amount: 'fixed', value }, allocation: 'divided' }, kind: 'all',
      to: [{ ref: 'any-target', count: { amount: 'chosen', minimum: 0 } }] }, duration: { duration: 'this-turn' } });
    assert.deepEqual(abilities('Prevent the next 4 damage that would be dealt this turn to any number of targets, divided as you choose.'),
      [{ kind: 'spell', effects: [{ steps: [divided(4)] }] }]);
    assert.deepEqual(abilities('Kicker {3}\nPrevent the next 3 damage that would be dealt this turn to any number of targets, divided as you choose. If this spell was kicked, prevent the next 6 damage this way instead.')[1],
      { kind: 'spell', effects: [{ steps: [{ effect: 'self-replacement', original: divided(3),
        condition: { condition: 'kicked', what: { ref: 'this', noun: 'spell' }, comparison: { op: 'ge', value: { amount: 'fixed', value: 1 } } },
        instead: [divided(6)], targetPolicy: 'retain-original' }] }] });
    // Division needs targets and a damage budget (601.2d).
    fails('Prevent all damage that would be dealt this turn to any number of targets, divided as you choose.');
    fails('Prevent the next 4 damage that would be dealt this turn to each creature, divided as you choose.');
    // "This way" restates a budget, not a different recipient.
    fails('Kicker {3}\nPrevent the next 3 damage that would be dealt to any target this turn. If this spell was kicked, prevent the next 6 damage this way to target creature instead.');
  });
  it('rejects unsupported riders', () => {
    // A static prevention's own instruction still can't target.
    fails('If damage would be dealt to this creature, prevent that damage. If damage is prevented this way, this creature deals that much damage to any target.', 'Creature — Horror');
    // A "this turn" trigger limit can't be invented for a static shield.
    fails('If damage would be dealt to this creature, prevent that damage. Whenever damage is prevented this way this turn, you gain that much life.', 'Creature — Horror');
    // A different graveyard or "up to" is not the per-damage quantity.
    fails('If damage would be dealt to you, prevent that damage. Exile a card from an opponent\'s graveyard for each 1 damage prevented this way.', 'Artifact');
    // The reflexive trigger body can't have its own condition.
    fails('Prevent all damage that would be dealt to you this turn. When damage is prevented this way, if you control a Cleric, you gain that much life.');
  });
});
