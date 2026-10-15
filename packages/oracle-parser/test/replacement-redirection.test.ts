import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// Damage redirection (CR 614.9), checked against the frozen Scryfall corpus:
// Carom, Nomads en-Kor, Harm's Way, Shining Shoal, Pariah, Palisade Giant,
// Martyrs of Korlis, Kjeldoran Royal Guard, Mirror Strike, Reverberation,
// Treacherous Link, Beacon of Destiny, Reflect Damage, Glarecaster,
// Soltari Guerrillas, Shield Dancer, Aegis of Honor, Harsh Judgment,
// Ascent of the Worthy, Gideon's Sacrifice.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed' as const, value });
const you = { player: 'you' };
const thisCreature = { ref: 'this', noun: 'creature' };
const thisTurn = { duration: 'this-turn' };
const all = { sources: 'simultaneous', recipients: 'simultaneous' };
const eachEvent = { shield: 'events', occurrence: 'each' };
const nextEvent = { shield: 'events', occurrence: 'next' };
const sourceChoice = { choice: 'damage-source', who: you, filter: { cls: 'source' }, when: 'effect-created', universe: 'damage-sources', recheck: 'when-damage-would-be-dealt' };
const eventSource = { ref: 'replacement-event-object', role: 'source' };
const target = (filter: object) => ({ ref: 'target', filter, count: n(1) });
function abilities(text: string, typeLine = 'Instant') {
  const parsed = parseOracleText(text, 'Test Card', { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities)) as unknown[];
}
const spell = (redirection: object, duration: object = thisTurn) => [{ kind: 'spell', effects: [{ steps: [{ effect: 'redirect-damage', redirection, duration }] }] }];
const fails = (text: string, typeLine = 'Instant') => assert.equal(parseOracleText(text, 'Test Card', { typeLine }).ok, false, text);

describe('damage redirection budgets ("the next N damage … is dealt to … instead")', () => {
  it('redirects a fixed budget between two targets (Carom)', () => {
    assert.deepEqual(abilities('The next 1 damage that would be dealt to target creature this turn is dealt to another target creature instead.'), spell({
      eventScope: all, kind: 'all', to: [target({ types: ['creature'] })], shield: { shield: 'damage-budget', amount: n(1) },
      redirectTo: target({ types: ['creature'], other: true }) }));
  });
  it('keeps the chosen source, the recipient union and an X budget (Shining Shoal)', () => {
    assert.deepEqual(abilities('The next X damage that a source of your choice would deal to you and/or creatures you control this turn is dealt to any target instead.'), spell({
      eventScope: all, kind: 'all', by: sourceChoice, to: [you, { ref: 'each', filter: { types: ['creature'], control: 'you' } }],
      shield: { shield: 'damage-budget', amount: { amount: 'x' } }, redirectTo: { ref: 'any-target' } }));
  });
  it('is an activated ability body with the source as original recipient (Nomads en-Kor)', () => {
    assert.deepEqual(abilities('{0}: The next 1 damage that would be dealt to this creature this turn is dealt to target creature you control instead.', 'Creature — Kor'), [{
      kind: 'activated', costs: [{ cost: 'mana', symbols: ['0'] }], restriction: undefined, effects: [{ steps: [{ effect: 'redirect-damage', redirection: {
        eventScope: all, kind: 'all', to: [thisCreature], shield: { shield: 'damage-budget', amount: n(1) },
        redirectTo: target({ types: ['creature'], control: 'you' }) }, duration: thisTurn }] }] }].map(a => JSON.parse(JSON.stringify(a))));
  });
});

describe('redirection of all matching damage', () => {
  it('is a static ability without a duration (Pariah, Palisade Giant)', () => {
    assert.deepEqual(abilities('All damage that would be dealt to you is dealt to enchanted creature instead.', 'Enchantment — Aura'), [{ kind: 'static', effect: {
      static: 'redirect-damage', redirection: { eventScope: all, kind: 'all', to: [you], shield: eachEvent, redirectTo: { ref: 'enchanted', noun: 'creature' } } } }]);
    assert.deepEqual(abilities('All damage that would be dealt to you and other permanents you control is dealt to this creature instead.', 'Creature — Giant'), [{ kind: 'static', effect: {
      static: 'redirect-damage', redirection: { eventScope: all, kind: 'all', to: [you, { ref: 'each', filter: { other: true, cls: 'permanent', control: 'you' } }],
        shield: eachEvent, redirectTo: thisCreature } } }]);
  });
  it('keeps a continuous condition, the damage kind and the sources (Weathered Bodyguards)', () => {
    assert.deepEqual(abilities('As long as this creature is untapped, all combat damage that would be dealt to you by unblocked creatures is dealt to this creature instead.', 'Creature — Human'), [{
      kind: 'static', effect: { static: 'conditional', condition: { condition: 'status', what: thisCreature, status: 'untapped' }, effect: { static: 'redirect-damage', redirection: {
        eventScope: all, kind: 'combat', to: [you], by: { ref: 'each', filter: { status: ['unblocked'], types: ['creature'] } }, shield: eachEvent, redirectTo: thisCreature } } } }]);
  });
  it('is a resolving effect with its printed duration (Sivvi\'s Valor, Ascent of the Worthy)', () => {
    assert.deepEqual(abilities('All damage that would be dealt to target creature this turn is dealt to you instead.'), spell({
      eventScope: all, kind: 'all', to: [target({ types: ['creature'] })], shield: eachEvent, redirectTo: you }));
    assert.deepEqual(abilities('Until your next turn, all damage that would be dealt to creatures you control is dealt to this creature instead.', 'Sorcery'), spell({
      eventScope: all, kind: 'all', to: [{ ref: 'each', filter: { types: ['creature'], control: 'you' } }], shield: eachEvent, redirectTo: thisCreature }, { duration: 'your-next-turn' }));
  });
  it('binds a pronoun for the damage source or its original recipient (Mirror Strike, Reverberation, Treacherous Link)', () => {
    const attacker = target({ status: ['unblocked'], types: ['creature'] });
    assert.deepEqual(abilities('All combat damage that would be dealt to you this turn by target unblocked creature is dealt to its controller instead.'), spell({
      eventScope: all, kind: 'combat', to: [you], by: attacker, shield: eachEvent, redirectTo: { player: 'controller', of: eventSource } }));
    assert.deepEqual(abilities('All damage that would be dealt this turn by target sorcery spell is dealt to that spell\'s controller instead.'), spell({
      eventScope: all, kind: 'all', by: target({ types: ['sorcery'], cls: 'spell' }), shield: eachEvent, redirectTo: { player: 'controller', of: eventSource } }));
    assert.deepEqual(abilities('All damage that would be dealt to enchanted creature is dealt to its controller instead.', 'Enchantment — Aura'), [{ kind: 'static', effect: {
      static: 'redirect-damage', redirection: { eventScope: all, kind: 'all', to: [{ ref: 'enchanted', noun: 'creature' }], shield: eachEvent,
        redirectTo: { player: 'controller', of: { ref: 'replacement-event-object', role: 'recipient' } } } } }]);
  });
});

describe('redirection of the next damage event', () => {
  const next = { sources: 'each', recipients: 'simultaneous' };
  it('redirects one instance from a chosen source (Beacon of Destiny, Reflect Damage)', () => {
    assert.deepEqual(abilities('The next time a source of your choice would deal damage to you this turn, that damage is dealt to target creature you control instead.'), spell({
      eventScope: next, kind: 'all', by: sourceChoice, to: [you], shield: nextEvent, redirectTo: target({ types: ['creature'], control: 'you' }) }));
    assert.deepEqual(abilities('The next time a source of your choice would deal damage this turn, that damage is dealt to that source\'s controller instead.'), spell({
      eventScope: next, kind: 'all', by: sourceChoice, shield: nextEvent, redirectTo: { player: 'controller', of: eventSource } }));
  });
  it('watches damage from any source to a recipient union (Glarecaster)', () => {
    assert.deepEqual(abilities('The next time damage would be dealt to this creature and/or you this turn, that damage is dealt to any target instead.'), spell({
      eventScope: all, kind: 'all', to: [thisCreature, you], shield: nextEvent, redirectTo: { ref: 'any-target' } }));
  });
  it('reads the active form as the same redirection, its subject being the source (Soltari Guerrillas, Shield Dancer, Aegis of Honor)', () => {
    assert.deepEqual(abilities('The next time this creature would deal combat damage to an opponent this turn, it deals that damage to target creature instead.'), spell({
      eventScope: next, kind: 'combat', by: thisCreature, to: [{ player: 'an-opponent' }], shield: nextEvent, redirectTo: target({ types: ['creature'] }) }));
    assert.deepEqual(abilities('The next time target attacking creature would deal combat damage to this creature this turn, that creature deals that damage to itself instead.'), spell({
      eventScope: next, kind: 'combat', by: target({ status: ['attacking'], types: ['creature'] }), to: [thisCreature], shield: nextEvent, redirectTo: eventSource }));
    const spells = { ref: 'filter', filter: { allOf: [{ anyOf: [{ types: ['instant'] }, { types: ['sorcery'] }] }], cls: 'spell' }, count: n(1) };
    assert.deepEqual(abilities('The next time an instant or sorcery spell would deal damage to you this turn, that spell deals that damage to its controller instead.'), spell({
      eventScope: next, kind: 'all', by: spells, to: [you], shield: nextEvent, redirectTo: { player: 'controller', of: eventSource } }));
  });
  it('is a static ability for every matching event (Harsh Judgment)', () => {
    assert.deepEqual(abilities('If a red spell would deal damage to you, it deals that damage to its controller instead.', 'Enchantment'), [{ kind: 'static', effect: {
      static: 'redirect-damage', redirection: { eventScope: { sources: 'each', recipients: 'each' }, kind: 'all',
        by: { ref: 'filter', filter: { colors: ['red'], cls: 'spell' }, count: n(1) }, to: [you], shield: eachEvent, redirectTo: { player: 'controller', of: eventSource } } } }]);
  });
});

describe('redirection boundaries', () => {
  it('leaves quantity changes and prevention to their own nodes', () => {
    const [doubled] = abilities('If a source would deal damage to you, it deals double that damage to you instead.', 'Enchantment') as { effect: { static: string; replacement: { replace: { replacement: string } } } }[];
    assert.equal(doubled.effect.static, 'replacement');
    assert.equal(doubled.effect.replacement.replace.replacement, 'modify-quantity');
    const [prevented] = abilities('The next time a source of your choice would deal damage to you this turn, prevent that damage.') as { effects: { steps: { effect: string }[] }[] }[];
    assert.equal(prevented.effects[0].steps[0].effect, 'prevent-damage');
  });
  it('rejects incomplete or unrepresentable redirections', () => {
    // No new recipient, the same recipient, and a missing "instead".
    fails('All damage that would be dealt to you this turn is dealt instead.');
    fails('All damage that would be dealt to you this turn is dealt to you instead.');
    fails('All damage that would be dealt to you this turn is dealt to target creature.');
    // A resolving effect that redirects everything needs its duration.
    fails('All damage that would be dealt to target creature is dealt to you instead.');
    // A static ability has no target, budget, single event or chosen source.
    fails('All damage that would be dealt to you is dealt to target creature instead.', 'Enchantment');
    fails('The next 1 damage that would be dealt to you is dealt to this creature instead.', 'Creature — Kor');
    fails('If a source of your choice would deal damage to you, that damage is dealt to this creature instead.', 'Creature — Kor');
    // The active subject must be the watched source; "that player" has no antecedent.
    fails('The next time this creature would deal combat damage to an opponent this turn, target creature deals that damage to you instead.');
    fails('The next time a source of your choice would deal damage this turn, that damage is dealt to that player instead.');
    // "Its" is ambiguous between two singular participants.
    fails('The next time target creature would deal damage to another target creature this turn, that damage is dealt to its controller instead.');
  });
});
