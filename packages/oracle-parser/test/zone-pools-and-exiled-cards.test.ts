import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// Oracle wordings checked against the frozen corpus; rules from the
// Comprehensive Rules of 2026-09-25.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function abilities(text: string, typeLine = 'Sorcery') {
  const r = parseOracleText(text, undefined, { typeLine });
  assert.ok(r.ok, JSON.stringify(r.lines));
  return JSON.parse(JSON.stringify(r.abilities));
}
const fails = (text: string, typeLine = 'Sorcery') => assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);

const GRAVE = { zone: 'graveyard', owner: 'your' };

describe('counted pools in different zones (CR 400.7)', () => {
  const pt = (forEach: object) => [{ kind: 'static', effect: { static: 'modify', what: { ref: 'this', noun: 'creature' },
    modifiers: [{ modifier: 'pt', power: { sign: 1, amount: { amount: 'fixed', value: 1 } },
      toughness: { sign: 1, amount: { amount: 'fixed', value: 1 } }, forEach }] } }];

  it('counts battlefield permanents and graveyard cards together (Moon-Vigil Adherents)', () => {
    assert.deepEqual(abilities('This creature gets +1/+1 for each creature you control and each creature card in your graveyard.', 'Creature — Elf Druid'),
      pt({ anyOf: [{ types: ['creature'], control: 'you' }, { types: ['creature'], cls: 'card', zone: GRAVE }] }));
  });

  it('binds "other" to its own pool (Desmond Miles)', () => {
    assert.deepEqual(abilities('~ gets +1/+0 for each other Assassin you control and each Assassin card in your graveyard.', 'Creature — Human Assassin'),
      [{ kind: 'static', effect: { static: 'modify', what: { ref: 'self' }, modifiers: [{ modifier: 'pt',
        power: { sign: 1, amount: { amount: 'fixed', value: 1 } }, toughness: { sign: 1, amount: { amount: 'fixed', value: 0 } },
        forEach: { anyOf: [{ subtypes: ['Assassin'], control: 'you', otherThan: { ref: 'self' } },
          { subtypes: ['Assassin'], cls: 'card', zone: GRAVE }] } }] } }]);
  });

  it('takes an extremum over both pools (Ambitious Dragonborn)', () => {
    assert.deepEqual(abilities('This creature enters with X +1/+1 counters on it, where X is the greatest power among creatures you control and creature cards in your graveyard.', 'Creature — Dragon Barbarian'),
      [{ kind: 'static', effect: { static: 'enters-with-counters', what: { ref: 'this', noun: 'creature' }, counter: { type: '+1/+1' },
        count: { amount: 'x' }, bindings: [{ variable: 'x', value: { amount: 'attribute-extremum', extremum: 'maximum', attribute: 'power',
          among: { anyOf: [{ types: ['creature'], control: 'you' }, { types: ['creature'], cls: 'card', zone: GRAVE }] } } }] } }]);
  });

  it('rejects pools that can share an object, and a second pool without its determiner', () => {
    fails('This creature gets +1/+1 for each creature you control and each creature you control.', 'Creature — Elf Druid');
    fails('This creature gets +1/+1 for each creature card in your graveyard and each artifact card in your graveyard.', 'Creature — Elf Druid');
    fails('This creature gets +1/+1 for each creature you control and creature card in your graveyard.', 'Creature — Elf Druid');
  });
});

describe('exiled cards as a description (CR 406.1)', () => {
  it('means cards in the exile zone, with face-up status and ownership kept', () => {
    assert.deepEqual(abilities('Put target face-up exiled card into its owner\'s graveyard.', 'Instant'),
      [{ kind: 'spell', effects: [{ steps: [{ effect: 'move-zone', what: { ref: 'target',
        filter: { status: ['face-up'], zone: { zone: 'exile' }, cls: 'card' }, count: { amount: 'fixed', value: 1 } },
      to: { zone: 'graveyard', owner: 'its-owner' } }] }] }]);
    assert.deepEqual(abilities('{T}, Sacrifice this artifact: Choose up to seven face-up exiled cards you own. Exile all the cards from your library, then put the chosen cards on top of your library.', 'Artifact')[0].effects[0].steps[0],
      { effect: 'choose-object', who: { player: 'you' }, what: { ref: 'filter',
        filter: { status: ['face-up'], zone: { zone: 'exile' }, cls: 'card', own: 'you' }, count: { amount: 'fixed', value: 7 }, upTo: true } });
  });

  it('keeps counters on the exiled cards', () => {
    assert.deepEqual(abilities('Return all exiled cards with aegis counters on them to the battlefield under their owners\' control.'),
      [{ kind: 'spell', effects: [{ steps: [{ effect: 'move-zone', what: { ref: 'all', filter: { zone: { zone: 'exile' }, cls: 'card',
        counterConstraints: [{ counter: { type: 'aegis' }, comparison: { op: 'ge', value: { amount: 'fixed', value: 1 } } }] } },
      to: { zone: 'battlefield' }, controller: 'owner' }] }] }]);
  });

  it('rejects exiled objects that are not cards', () => {
    fails('Exile target exiled creature.');
    fails('Destroy target exiled permanent.');
  });
});

describe('a closing card head and zone cover every alternative (CR 109.2a, 400.1)', () => {
  const grave = { zone: 'graveyard', owner: 'your' };
  const ret = (what: object) => [{ kind: 'spell', effects: [{ steps: [{ effect: 'return-to-hand', what, to: 'your' }] }] }];
  const one = { amount: 'fixed', value: 1 };

  it('puts the zone on each alternative (Haven of the Spirit Dragon)', () => {
    assert.deepEqual(abilities('Return target Dragon creature card or Ugin planeswalker card from your graveyard to your hand.'),
      ret({ ref: 'target', count: one, filter: { anyOf: [
        { subtypes: ['Dragon'], types: ['creature'], cls: 'card', zone: grave },
        { subtypes: ['Ugin'], types: ['planeswalker'], cls: 'card', zone: grave }] } }));
  });

  it('gives a bare first alternative the shared card head (Abuelo\'s Awakening)', () => {
    assert.deepEqual(abilities('Return target artifact or non-Aura enchantment card from your graveyard to your hand.'),
      ret({ ref: 'target', count: one, filter: { anyOf: [
        { types: ['artifact'], cls: 'card', zone: grave },
        { nonSubtypes: ['Aura'], types: ['enchantment'], cls: 'card', zone: grave }] } }));
  });

  it('works with repeated articles for a non-targeted choice (Overlord of the Balemurk)', () => {
    assert.deepEqual(abilities('Return a non-Avatar creature card or a planeswalker card from your graveyard to your hand.'),
      ret({ ref: 'filter', count: one, filter: { anyOf: [
        { nonSubtypes: ['Avatar'], types: ['creature'], cls: 'card', zone: grave },
        { types: ['planeswalker'], cls: 'card', zone: grave }] } }));
  });

  it('leaves a trailing qualifier on one alternative unsupported', () => {
    fails('Return target creature card with flying or enchantment card from your graveyard to your hand.');
    fails('Return target artifact or non-Aura enchantment card with mana value 3 or less from your graveyard to your hand.');
  });
});

describe('a two-zone origin followed by another instruction', () => {
  it('ends the zone phrase at "and" when a verb follows (Memory Leak)', () => {
    const steps = abilities('Target opponent reveals their hand. You choose a nonland card from that player\'s graveyard or hand and exile it.')[0].effects[1].steps;
    assert.deepEqual(steps, [{ effect: 'choose-object', who: { player: 'you' }, what: { ref: 'filter', count: { amount: 'fixed', value: 1 },
      filter: { nonTypes: ['land'], cls: 'card', anyOf: [{ zone: { zone: 'graveyard', owner: 'that-player' } }, { zone: { zone: 'hand', owner: 'that-player' } }] } } },
    { effect: 'exile', what: { ref: 'it' } }]);
  });

  it('still rejects a third zone', () => {
    fails('Exile a creature card from your hand or graveyard and library.');
  });
});
