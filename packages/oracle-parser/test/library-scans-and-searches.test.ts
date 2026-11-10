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

const YOUR_LIBRARY = { zone: 'library', owner: { player: 'you' } };

describe('search with a where-X definition before the put (CR 701.23, 107.3)', () => {
  it('keeps the put and the shuffle in the same chain (Harvest Season)', () => {
    assert.deepEqual(abilities('Search your library for up to X basic land cards, where X is the number of tapped creatures you control, put those cards onto the battlefield tapped, then shuffle.'), [
      { kind: 'spell', effects: [{ steps: [
        { effect: 'search', who: { player: 'you' }, zones: { selection: 'all', of: [{ zone: 'library', owner: 'your' }] },
          for: { ref: 'filter', filter: { supertypes: ['basic'], types: ['land'], cls: 'card' }, count: { amount: 'x' }, upTo: true } },
        { effect: 'move-zone', what: { ref: 'that', filter: { cls: 'card' }, plural: true }, to: { zone: 'battlefield' }, tapped: true },
        { effect: 'shuffle', who: { player: 'you' }, library: YOUR_LIBRARY },
      ], bindings: [{ variable: 'x', value: { amount: 'count', of: { status: ['tapped'], types: ['creature'], control: 'you' } } }] }] },
    ]);
  });
  it('does not continue a non-search instruction without "then"', () => {
    fails('Draw X cards, where X is the number of tapped creatures you control, put a card from your hand on the bottom of your library.');
  });
});

describe('those players (CR 608.2c)', () => {
  it('is the plural counterpart of that player, and its shuffle is theirs (Gomazoa-style)', () => {
    assert.deepEqual(abilities('{T}: Put this creature and target creature on top of their owners\' libraries, then those players shuffle their libraries.', 'Creature — Jellyfish')[0].effects[0].steps[1],
      { effect: 'shuffle', who: { player: 'those-players' }, library: { zone: 'library', owner: 'their' } });
  });
  it('carries the plural actor to an imperative continuation', () => {
    const steps = abilities('Those players put those cards onto the battlefield, then shuffle.')[0].effects[0].steps;
    assert.deepEqual(steps.map((s: { who: object }) => s.who), [{ player: 'those-players' }, { player: 'those-players' }]);
  });
  it('does not make an imperative after a new sentence theirs', () => {
    assert.deepEqual(abilities('Those players put those cards onto the battlefield. Shuffle.')[0].effects[1].steps[0].who, { player: 'you' });
  });
});

describe('library scan with a passive stop condition (CR 701.20, 609.3)', () => {
  it('stops when N matching cards are revealed (Mind Funeral)', () => {
    assert.deepEqual(abilities('Target opponent reveals cards from the top of their library until four land cards are revealed.')[0].effects[0].steps[0],
      { effect: 'scan-library', who: { player: 'target-opponent' }, operation: 'reveal', library: { zone: 'library', owner: 'their' },
        until: { count: { amount: 'fixed', value: 4 }, matching: { types: ['land'], cls: 'card' } } });
  });
  it('is the same node as the active wording', () => {
    const passive = abilities('Reveal cards from the top of your library until a creature card is revealed.')[0].effects[0].steps[0];
    const active = abilities('Reveal cards from the top of your library until you reveal a creature card.')[0].effects[0].steps[0];
    assert.deepEqual(passive, active);
  });
  it('requires the passive verb to match the operation', () => {
    fails('Reveal cards from the top of your library until a creature card is exiled.');
    fails('Exile cards from the top of your library until a creature card is revealed.');
  });
});

describe('investigating once per counted player (CR 701.16a)', () => {
  it('repeats the action once for each opponent with more cards in hand (Wojek Investigator)', () => {
    assert.deepEqual(abilities('Investigate once for each opponent who has more cards in hand than you.')[0].effects[0].steps[0], {
      effect: 'keyword-action', action: 'investigate', who: { player: 'you' },
      times: { amount: 'player-count', of: { player: 'filtered', base: { player: 'each-opponent' }, filter: {
        handSize: { op: 'gt', value: { amount: 'count', of: { cls: 'card', zone: { zone: 'hand', owner: 'your' } } } } } } } });
  });
  it('keeps the object form on forEach', () => {
    assert.deepEqual(abilities('Investigate once for each creature you control.')[0].effects[0].steps[0],
      { effect: 'keyword-action', action: 'investigate', who: { player: 'you' }, forEach: { types: ['creature'], control: 'you' } });
  });
  it('needs a qualified player set', () => {
    fails('Investigate once for each opponent.');
  });
});

describe('the number of your opponents with a hand size (CR 102.2)', () => {
  it('counts the opponents whose hand has four or more cards (Wolfcaller\'s Howl)', () => {
    assert.deepEqual(abilities('Create X 2/2 green Wolf creature tokens, where X is the number of your opponents with four or more cards in hand.')[0].effects[0].steps[0].count, { amount: 'x' });
    assert.deepEqual(abilities('Create X 2/2 green Wolf creature tokens, where X is the number of your opponents with four or more cards in hand.')[0].effects[0].bindings, [
      { variable: 'x', value: { amount: 'player-count', of: { player: 'filtered', base: { player: 'each-opponent' },
        filter: { handSize: { op: 'ge', value: { amount: 'fixed', value: 4 } } } } } }]);
  });
  it('rejects "your players"', () => {
    fails('Create X 2/2 green Wolf creature tokens, where X is the number of your players with four or more cards in hand.');
  });
});

describe('"other than <kind>" on a selection (CR 608.2c)', () => {
  const basicLand = { supertypes: ['basic'], types: ['land'], cls: 'card' };
  it('excludes a plain subtype', () => {
    assert.deepEqual(abilities('Destroy all creatures other than Zombies and Vampires.')[0].effects[0].steps[0].what.filter,
      { types: ['creature'], nonSubtypes: ['Zombie', 'Vampire'] });
  });
  it('excludes a compound kind as noneOf (Extirpate)', () => {
    assert.deepEqual(abilities('Exile target card in a graveyard other than a basic land card.')[0].effects[0].steps[0].what.filter,
      { cls: 'card', zone: { zone: 'graveyard', owner: 'any' }, noneOf: [basicLand] });
  });
  it('reads a trailing zone as the selection\'s own (Kotose)', () => {
    assert.deepEqual(abilities('Exile target card other than a basic land card from an opponent\'s graveyard.')[0].effects[0].steps[0].what.filter,
      { cls: 'card', zone: { zone: 'graveyard', owner: 'an-opponent' }, noneOf: [basicLand] });
  });
  it('rejects excluding the selected type itself and scoped exclusions', () => {
    fails('Destroy all creatures other than a creature.');
    fails('Destroy all creatures other than creatures you control.');
  });
});

describe('looking at X cards then putting them back (CR 701.20e)', () => {
  it('keeps the where-X definition out of the looked-at library (Descendant of Soramaro)', () => {
    assert.deepEqual(abilities('Look at the top X cards of your library, where X is the number of cards in your hand, then put them back in any order.')[0].effects[0], {
      steps: [
        { effect: 'look-at', who: { player: 'you' }, what: { top: { amount: 'x' }, of: { zone: 'library', owner: 'your' } } },
        { effect: 'move-zone', what: { ref: 'them' }, to: { zone: 'library', owner: 'your', position: 'top' }, order: 'any' },
      ],
      bindings: [{ variable: 'x', value: { amount: 'count', of: { cls: 'card', zone: { zone: 'hand', owner: 'your' } } } }],
    });
  });
});

describe('exiled cards used to craft it (CR 702.167c)', () => {
  const materials = { ref: 'related', number: 'plural', filter: { cls: 'card' }, relation: { relation: 'craft-material', of: { ref: 'it' } } };
  it('sums the materials\' power (Saheeli\'s Lattice)', () => {
    assert.deepEqual(abilities('~\'s power is equal to the total power of the exiled cards used to craft it.', 'Artifact Creature — Dinosaur'),
      [{ kind: 'static', effect: { static: 'cda-pt', what: { ref: 'self' }, stat: 'power',
        value: { amount: 'attribute-sum', attribute: 'power', among: materials } } }]);
  });
  it('counts colors among the materials (Sunbird Standard)', () => {
    assert.deepEqual(abilities('~\'s power and toughness are each equal to the number of colors among the exiled cards used to craft it.', 'Artifact Creature — Bird Construct')[0].effect.value,
      { amount: 'distinct-count', characteristic: 'color', among: materials });
  });
  it('uses one material\'s mana value (Jade Seedstones)', () => {
    assert.deepEqual(abilities('When this creature enters, you gain life equal to the mana value of the exiled card used to craft it.', 'Artifact Creature — Golem')[0].effects[0].steps[0].amount,
      { amount: 'attribute', of: { ref: 'related', number: 'singular', filter: { cls: 'card' }, relation: { relation: 'craft-material', of: { ref: 'it' } } }, attribute: 'mana-value' });
  });
  it('selects an exiled creature card as a craft material (Throne of the Grim Captain)', () => {
    const step = abilities('Whenever ~ attacks, you may put an exiled creature card used to craft ~ onto the battlefield.', 'Legendary Artifact')[0].effects[0].steps[0];
    assert.deepEqual(step.what.filter, { zone: { zone: 'exile' }, types: ['creature'], cls: 'card', relation: { relation: 'craft-material', of: { ref: 'self' } } });
  });
  it('is not the exile link of another object', () => {
    fails('~\'s power is equal to the total power of the exiled cards used to craft.');
  });
});
