import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// Self-replacement riders (CR 614.15, 608.2c) checked against the frozen
// Scryfall corpus: Brimstone Volley, That's Rough Buddy, Skeletal Swarming,
// Festering Newt, Dissipate, Memory Lapse, Desertion, No More Lies.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed' as const, value });
const creature = { types: ['creature'] };
const target = { ref: 'target', filter: creature, count: n(1) };
const morbid = { condition: 'zone-change-history', event: 'dies', subject: { match: 'filter', filter: creature, comparison: { op: 'ge', value: n(1) } },
  from: { zone: 'battlefield' }, to: { zone: 'graveyard' }, during: 'this-turn' };
const island = { condition: 'control', who: { player: 'you' }, what: { ref: 'filter', filter: { subtypes: ['Island'] }, count: n(1) } };
function card(text: string, typeLine = 'Instant') {
  const parsed = parseOracleText(text, 'Test Card', { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed)) as { lines: { text: string }[]; abilities: unknown[] };
}
const fails = (text: string, typeLine = 'Instant') => assert.equal(parseOracleText(text, 'Test Card', { typeLine }).ok, false, text);

describe('self-replacement with its condition printed last', () => {
  it('reads "X instead if C" as the same replacement as "If C, X instead"', () => {
    const original = { effect: 'put-counters', on: target, counter: { type: '+1/+1' }, count: n(1) };
    const instead = [{ effect: 'put-counters', on: { ref: 'that', filter: creature }, counter: { type: '+1/+1' }, count: n(2) }];
    const trailing = card('Put a +1/+1 counter on target creature. Put two +1/+1 counters on that creature instead if a creature died this turn.');
    assert.deepEqual(trailing.abilities, [{ kind: 'spell', effects: [{ steps: [{ effect: 'self-replacement', original, condition: morbid, instead, targetPolicy: 'retain-original' }] }] }]);
    assert.deepEqual(trailing.abilities, card('Put a +1/+1 counter on target creature. If a creature died this turn, put two +1/+1 counters on that creature instead.').abilities);
  });
  it('keeps the announced recipient for an abbreviated damage replacement', () => {
    const to = { to: 'any-target' };
    assert.deepEqual(card('Test Card deals 3 damage to any target. It deals 5 damage instead if you control an Island.').abilities, [{ kind: 'spell', effects: [{ steps: [{
      effect: 'self-replacement', original: { effect: 'damage', source: { ref: 'self' }, amount: n(3), to }, condition: island,
      instead: [{ effect: 'damage', source: { ref: 'it' }, amount: n(5), to }], targetPolicy: 'retain-original' }] }] }]);
  });
  it('rejects a second instead, an unparsed condition, and a later ordinary condition', () => {
    fails('Draw a card. Draw two cards instead if you control an Island instead.');
    fails('Draw a card. Draw two cards instead if the moon is full.');
    fails('Draw a card. Draw two cards instead.');
    // An ordinary trailing condition is not a replacement of the first draw.
    assert.deepEqual(card('Draw a card. Draw two cards if you control an Island.').abilities, [{ kind: 'spell', effects: [
      { steps: [{ effect: 'draw', who: { player: 'you' }, amount: n(1) }] },
      { steps: [{ effect: 'draw', who: { player: 'you' }, amount: n(2) }], condition: island, conditionKind: 'if' }] }]);
  });
});

describe('replacement printed as its own paragraph', () => {
  it('forms one block with the replaced spell ability and keeps the ability word', () => {
    const to = { to: 'any-target' };
    const parsed = card('Test Card deals 3 damage to any target.\nMorbid — Test Card deals 5 damage instead if a creature died this turn.');
    assert.deepEqual(parsed.lines.map(line => line.text), ['~ deals 3 damage to any target.\nMorbid — ~ deals 5 damage instead if a creature died this turn.']);
    assert.deepEqual(parsed.abilities, [{ kind: 'spell', effects: [{ steps: [{ effect: 'self-replacement',
      original: { effect: 'damage', source: { ref: 'self' }, amount: n(3), to }, condition: morbid,
      instead: [{ effect: 'damage', source: { ref: 'self' }, amount: n(5), to }], targetPolicy: 'retain-original', abilityWord: 'Morbid' }] }] }]);
  });
  it('accepts a leading condition and leaves later paragraphs independent', () => {
    const parsed = card('Draw three cards.\nAddendum — If you control an Island, instead scry 3, then draw three cards.\nFlashback {2}{U}');
    assert.deepEqual(parsed.abilities, [
      { kind: 'spell', effects: [{ steps: [{ effect: 'self-replacement', original: { effect: 'draw', who: { player: 'you' }, amount: n(3) }, condition: island,
        instead: [{ effect: 'scry', amount: n(3) }, { effect: 'draw', who: { player: 'you' }, amount: n(3) }], targetPolicy: 'retain-original', abilityWord: 'Addendum' }] }] },
      { kind: 'keywords', keywords: [{ keyword: 'flashback', cost: [{ cost: 'mana', symbols: ['2', 'U'] }] }] }]);
  });
  it('does not merge when the replaced part of the first paragraph is ambiguous', () => {
    // Two sentences, a conditional original, and a non-spell first paragraph.
    fails('Draw a card. You gain 1 life.\nMorbid — Draw two cards instead if a creature died this turn.');
    // A conditional original is now replaced as a whole by a sentence that
    // restates its own condition (Fatal Push), instead of being rejected.
    const manaValue = (value: number) => ({ condition: 'is-filter', what: { ref: 'it' }, filter: { properties: [{ property: 'mana-value', comparison: { op: 'le', value: n(value) } }] } });
    assert.deepEqual(card('Destroy target creature if it has mana value 2 or less.\nRevolt — Destroy that creature if it has mana value 4 or less instead if a creature died this turn.').abilities, [{
      kind: 'spell', effects: [{ steps: [{ effect: 'self-replacement',
        original: { sequence: [{ steps: [{ effect: 'destroy', what: target }], condition: manaValue(2), conditionKind: 'if' }] }, condition: morbid,
        instead: { sequence: [{ steps: [{ effect: 'destroy', what: { ref: 'that', filter: creature } }], condition: manaValue(4), conditionKind: 'if' }] },
        targetPolicy: 'retain-original', abilityWord: 'Revolt' }] }] }]);
    fails('Flying\nMorbid — Draw two cards instead if a creature died this turn.', 'Creature — Bird');
    fails('Draw a card.\nMorbid — Draw two cards instead if a creature died this turn. You gain 1 life.');
  });
});

describe('abbreviated replacements restate only what changes', () => {
  const token = { colors: ['black'], supertypes: [], types: ['creature'], subtypes: ['Skeleton'], keywords: [], power: n(1), toughness: n(1), tapped: true };
  it('creates N tokens of the replaced specification for "of those tokens"', () => {
    assert.deepEqual(card('Create a tapped 1/1 black Skeleton creature token. If a creature died this turn, create two of those tokens instead.', 'Sorcery').abilities, [{
      kind: 'spell', effects: [{ steps: [{ effect: 'self-replacement', original: { effect: 'create-token', count: n(1), token }, condition: morbid,
        instead: [{ effect: 'create-token', count: n(2), token }], targetPolicy: 'retain-original' }] }] }]);
    fails('Draw a card. If a creature died this turn, create two of those tokens instead.');
  });
  it('keeps the duration of a replaced pump, and nothing else', () => {
    const pump = (value: number) => ({ power: { sign: -1, amount: n(value) }, toughness: { sign: -1, amount: n(value) }, duration: { duration: 'end-of-turn' } });
    assert.deepEqual(card('Target creature gets -1/-1 until end of turn. That creature gets -4/-4 instead if you control an Island.').abilities, [{
      kind: 'spell', effects: [{ steps: [{ effect: 'self-replacement', original: { effect: 'pump', what: target, ...pump(1) }, condition: island,
        instead: [{ effect: 'pump', what: { ref: 'that', filter: creature }, ...pump(4) }], targetPolicy: 'retain-original' }] }] }]);
    // A different lasting effect without a printed duration is not inferred.
    fails('Target creature gets -1/-1 until end of turn. That creature gains flying instead if you control an Island.');
  });
});

describe('destination of a spell countered this way', () => {
  const spell = { ref: 'target', filter: { cls: 'spell' }, count: n(1) };
  const counter = (countered: unknown) => [{ kind: 'spell', effects: [{ steps: [{ effect: 'counter', what: spell, countered }] }] }];
  it('modifies the counter event rather than adding a later instruction', () => {
    assert.deepEqual(card("Counter target spell. If that spell is countered this way, exile it instead of putting it into its owner's graveyard.").abilities, counter({ to: { zone: 'exile' } }));
    assert.deepEqual(card("Counter target spell. If that spell is countered this way, put it on top of its owner's library instead of into that player's graveyard.").abilities,
      counter({ to: { zone: 'library', owner: 'its-owner', position: 'top' } }));
    assert.deepEqual(card("Counter target spell. If that spell is countered this way, put it into its owner's hand instead of into that player's graveyard.").abilities,
      counter({ to: { zone: 'hand', owner: 'its-owner' } }));
  });
  it('keeps an unless-payment on the counter and a type restriction on the destination', () => {
    assert.deepEqual(card("Counter target spell unless its controller pays {3}. If that spell is countered this way, exile it instead of putting it into its owner's graveyard.").abilities, [{
      kind: 'spell', effects: [{ steps: [{ effect: 'counter', what: spell, countered: { to: { zone: 'exile' } } }],
        condition: { condition: 'pays', who: { player: 'controller', of: { ref: 'it' } }, cost: [{ cost: 'mana', symbols: ['3'] }] }, conditionKind: 'unless' }] }]);
    assert.deepEqual(card("Counter target spell. If an artifact or creature spell is countered this way, put that card onto the battlefield under your control instead of into its owner's graveyard.").abilities,
      counter({ to: { zone: 'battlefield' }, controller: 'you', only: { allOf: [{ anyOf: [{ types: ['artifact'] }, { types: ['creature'] }] }], cls: 'spell' } }));
  });
  it('rejects near misses that state a different or incomplete destination', () => {
    // Without "instead of" this is a later conditional instruction on the
    // countered card, never a destination of the counter event itself.
    assert.deepEqual(card('Counter target spell. If that spell is countered this way, exile it.').abilities, [{ kind: 'spell', effects: [
      { steps: [{ effect: 'counter', what: spell }] },
      { steps: [{ effect: 'exile', what: { ref: 'it' } }], conditionKind: 'if', condition: { condition: 'is-filter', what: { ref: 'that', filter: { cls: 'spell' } },
        filter: { relation: { relation: 'event-result', event: 'counter', scope: 'this-way' } } } },
    ] }]);
    fails("Counter target spell. If that spell is countered this way, put it onto the battlefield instead of into its owner's graveyard.");
    fails("Counter target spell. If that spell is countered this way, put that card on your choice of the top or bottom of its owner's library instead of into that player's graveyard.");
    fails("Counter target spell. If that spell is countered this way, exile it instead of putting it into your graveyard.");
    fails("Destroy target creature. If that spell is countered this way, exile it instead of putting it into its owner's graveyard.");
    fails("Counter up to one target spell. If that spell is countered this way, exile it instead of putting it into its owner's graveyard.");
  });
});
