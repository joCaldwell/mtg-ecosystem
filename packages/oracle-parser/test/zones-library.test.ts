import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';
import { buildCardNameCatalog } from '../src/card-names.ts';

// Oracle wordings checked against the frozen corpus. CR 400.1/400.3/400.11
// (zones, owners, outside the game), 401.4/401.7 (library positions), 103.6a
// (opening hand), 100.2a/113.6n (deck construction), 701.20e, 701.24b.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed', value });
const you = { player: 'you' };
const card = { cls: 'card' };
const yourLibrary = { zone: 'library', owner: 'your' };
const yourHand = { zone: 'hand', owner: 'your' };
const yourGraveyard = { zone: 'graveyard', owner: 'your' };
const cardNames = buildCardNameCatalog([{ name: 'Relentless Rats', layout: 'normal' }, { name: 'Seven Dwarves', layout: 'normal' }]);
function abilities(text: string, typeLine = 'Sorcery') {
  const r = parseOracleText(text, undefined, { typeLine, cardNames });
  assert.ok(r.ok, JSON.stringify(r.lines));
  return JSON.parse(JSON.stringify(r.abilities));
}
const steps = (text: string, typeLine?: string) => abilities(text, typeLine).map((a: { effects: { steps: unknown }[] }) => a.effects.map(s => s.steps));
const fails = (text: string, typeLine = 'Sorcery') => assert.equal(parseOracleText(text, undefined, { typeLine, cardNames }).ok, false, text);

describe('quantified personal zones', () => {
  it('pools all graveyards, distinct from one graveyard per object', () => {
    assert.deepEqual(steps('You gain life equal to the number of creature cards in all graveyards.'), [[[{
      effect: 'gain-life', who: you, amount: { amount: 'count', of: { types: ['creature'], cls: 'card', zone: { zone: 'graveyard', owner: 'all-players' } } },
    }]]]);
    assert.deepEqual(steps('Exile target card from a graveyard.'), [[[{
      effect: 'exile', what: { ref: 'target', filter: { cls: 'card', zone: { zone: 'graveyard', owner: 'any' } }, count: n(1) },
    }]]]);
  });

  it('treats "all graveyards" as an object like each player\'s graveyard', () => {
    const expected = [[[{ effect: 'exile', what: { ref: 'zone-cards', zone: 'graveyard', owner: { player: 'each-player' } } }]]];
    assert.deepEqual(steps('Exile all graveyards.'), expected);
    assert.deepEqual(steps("Exile each player's graveyard."), expected);
  });

  it('requires targets from a single graveyard to share it', () => {
    assert.deepEqual(steps('Exile up to two target cards from a single graveyard.'), [[[{
      effect: 'exile', what: { ref: 'target', filter: { cls: 'card', zone: { zone: 'graveyard', owner: 'any-single' } }, count: n(2), upTo: true },
    }]]]);
  });

  it('distributes over each graveyard and pools opponents\' graveyards', () => {
    assert.deepEqual(steps('Exile a creature card from each graveyard.'), [[[{
      effect: 'exile', what: { ref: 'filter', filter: { types: ['creature'], cls: 'card', zone: { zone: 'graveyard', owner: 'each-player' } }, count: n(1) },
    }]]]);
    assert.deepEqual(steps("Draw a card for each creature card in your opponents' graveyards."), [[[{
      effect: 'draw', who: you, amount: n(1), forEach: { types: ['creature'], cls: 'card', zone: { zone: 'graveyard', owner: 'all-opponents' } },
    }]]]);
  });

  it('rejects number disagreement and unsupported zone quantifiers', () => {
    for (const text of ['Exile all cards from all graveyard.', 'Exile a card from each graveyards.', 'Exile two target cards from a single graveyards.', 'Exile all battlefields.'])
      fails(text);
  });
});

describe('zone-size conditions', () => {
  it('counts one unspecified graveyard, never the pooled graveyards', () => {
    assert.deepEqual(abilities('Draw a card if a graveyard has twenty or more cards in it.')[0].effects[0].condition, {
      condition: 'count', of: { cls: 'card', zone: { zone: 'graveyard', owner: 'any-single' } }, comparison: { op: 'ge', value: n(20) },
    });
  });

  it('keeps an opponent as the existential owner of the counted graveyard', () => {
    assert.deepEqual(abilities('Draw a card if an opponent has eight or more cards in their graveyard.')[0].effects[0].condition, {
      condition: 'count', of: { cls: 'card', zone: { zone: 'graveyard', owner: { player: 'an-opponent' } } }, comparison: { op: 'ge', value: n(8) },
    });
  });

  it('rejects a zone-size clause that names another zone or no cards', () => {
    fails('Draw a card if a graveyard has twenty or more cards in your hand.');
    fails('Draw a card if a graveyard has twenty or more creatures in it.');
    fails('Draw a card if each opponent has eight or more cards in their graveyard.');
  });
});

describe('contextual library placement', () => {
  const search = { effect: 'search', who: you, zones: { selection: 'all', of: [yourLibrary] }, for: { ref: 'filter', filter: card, count: n(1) } };
  const shuffle = { effect: 'shuffle', who: you, library: { zone: 'library', owner: you } };

  it('puts the found card on top of the library the same player just shuffled', () => {
    assert.deepEqual(steps('Search your library for a card, then shuffle and put that card on top.'), [[[search, shuffle, {
      effect: 'move-zone', what: { ref: 'that', filter: card }, to: { zone: 'library', owner: you, position: 'top' },
    }]]]);
    const target = { player: 'target-player' };
    assert.deepEqual(steps('Target player searches their library for a card, then shuffles and puts that card on top.')[0][0].slice(1), [
      { effect: 'shuffle', who: target, library: { zone: 'library', owner: target } },
      { effect: 'move-zone', what: { ref: 'that', filter: card }, to: { zone: 'library', owner: target, position: 'top' }, who: target },
    ]);
  });

  it('keeps the chosen order of several cards put on top', () => {
    assert.deepEqual(steps('Search your library for X cards, then shuffle and put those cards on top in any order.')[0][0][2], {
      effect: 'move-zone', what: { ref: 'that', filter: card, plural: true }, to: { zone: 'library', owner: you, position: 'top' }, order: 'any',
    });
  });

  it('puts looked-at cards back on the library that was looked at, not the actor\'s', () => {
    assert.deepEqual(steps('Look at the top three cards of your library, then put them back in any order.'), [[[
      { effect: 'look-at', who: you, what: { top: n(3), of: yourLibrary } },
      { effect: 'move-zone', what: { ref: 'them' }, to: { ...yourLibrary, position: 'top' }, order: 'any' },
    ]]]);
    const that = { player: 'that-player' };
    assert.deepEqual(steps('That player looks at the top three cards of your library, then puts them back in any order.'), [[[
      { effect: 'look-at', who: that, what: { top: n(3), of: yourLibrary } },
      { effect: 'move-zone', what: { ref: 'them' }, to: { ...yourLibrary, position: 'top' }, order: 'any', who: that },
    ]]]);
  });

  it('accepts a redundant "back" only before an explicit library position', () => {
    assert.deepEqual(steps('Look at the top three cards of your library. You may put one of those cards back on top of your library.')[0][1], [{
      effect: 'move-zone', what: { ref: 'from-pool', pool: { ref: 'that', filter: card, plural: true }, count: n(1) },
      to: { ...yourLibrary, position: 'top' }, optional: true,
    }]);
    fails('Look at the top three cards of your library. Put one of those cards back into your hand.');
  });

  it('shares the library of a preceding destination in the same put', () => {
    assert.deepEqual(steps('Put that card on top of your library and the rest on the bottom in any order.'), [[[{ effect: 'simultaneous', steps: [
      { effect: 'move-zone', what: { ref: 'that', filter: card }, to: { ...yourLibrary, position: 'top' } },
      { effect: 'move-zone', what: { ref: 'rest' }, to: { ...yourLibrary, position: 'bottom' }, order: 'any' },
    ] }]]]);
  });

  it('rejects elided libraries without their printed antecedent', () => {
    for (const text of [
      'Put that card on top.',
      'Search your library for a card, then put that card on top.',
      'Search your library for a card, then shuffle and put a card on top.',
      'Put them back in any order.',
      'Draw three cards, then put them back in any order.',
      'Look at the top three cards of your library, then put them back.',
      'Look at the top three cards of your library, then put them back in a random order.',
      'Put that card into your hand and the rest on the bottom in any order.',
    ]) fails(text);
  });
});

describe('library depth and split pairs', () => {
  it('records Nth from the top as part of the destination', () => {
    assert.deepEqual(steps("Put target creature into its owner's library second from the top."), [[[{
      effect: 'move-zone', what: { ref: 'target', filter: { types: ['creature'] }, count: n(1) },
      to: { zone: 'library', owner: 'its-owner', position: 'top', depth: 2 },
    }]]]);
    fails("Put target creature into its owner's library second from the bottom.");
    fails("Put target creature into its owner's graveyard second from the top.");
    fails("Put target creature on top of its owner's library third from the top.");
  });

  it('reads elliptical "one … the other" as one of them and the rest', () => {
    const split = (first: object, second: object) => ({ effect: 'simultaneous', steps: [
      { effect: 'move-zone', what: { ref: 'from-pool', pool: { ref: 'them' }, count: n(1) }, ...first },
      { effect: 'move-zone', what: { ref: 'rest' }, ...second },
    ] });
    assert.deepEqual(steps('Put one into your hand and the other into your graveyard.'), [[[split({ to: yourHand }, { to: yourGraveyard })]]]);
    assert.deepEqual(steps('Put one onto the battlefield tapped and the other into your hand.'),
      [[[split({ to: { zone: 'battlefield' }, tapped: true }, { to: yourHand })]]]);
    assert.deepEqual(steps('Put one of them into your hand and the other into your graveyard.'), [[[split({ to: yourHand }, { to: yourGraveyard })]]]);
  });

  it('keeps "the other" as the symbolic complement outside the one/other pair', () => {
    // A bare "the other" in an action slot is the contextual complement
    // (`rest`), bound later like any anaphor; see quantified-selections.
    assert.deepEqual(steps('Put the other into your hand.'), [[[{ effect: 'move-zone', what: { ref: 'rest' }, to: { zone: 'hand', owner: 'your' } }]]]);
    fails('Put one into your hand, one into your graveyard, and the other on the bottom of your library.');
  });
});

describe('zone moves', () => {
  it('mills a quantity stated after the noun', () => {
    assert.deepEqual(steps('Target player mills cards equal to its power.'), [[[{
      effect: 'mill', who: { player: 'target-player' }, amount: { amount: 'attribute', of: { ref: 'it' }, attribute: 'power' },
    }]]]);
    fails('Target player mills cards.');
  });

  it('keeps "their hand" symbolic for cards from one player\'s graveyard only', () => {
    assert.deepEqual(steps('Each player returns all creature cards from their graveyard to their hand.'), [[[{
      effect: 'return-to-hand', what: { ref: 'all', filter: { types: ['creature'], cls: 'card', zone: { zone: 'graveyard', owner: 'their' } } },
      to: 'their', who: { player: 'each-player' },
    }]]]);
    fails('Target player returns a creature they control to their hand.');
    fails('Return target creature to their hand.');
  });

  it('exiles one definite card with counters as part of the same move', () => {
    assert.deepEqual(steps('Exile ~ with three time counters on it.'), [[[{
      effect: 'exile', what: { ref: 'self' }, counters: [{ counter: { type: 'time' }, count: n(3) }],
    }]]]);
    // A counter qualifier on an indefinite selection describes the objects
    // to exile; it is not counters given to them in exile.
    assert.deepEqual(steps('Exile all creatures with three time counters on them.'), [[[{
      effect: 'exile', what: { ref: 'all', filter: { types: ['creature'], counterConstraints: [{ counter: { type: 'time' }, comparison: { op: 'eq', value: n(3) } }] } },
    }]]]);
    fails('Exile ~ with three time counters.');
    fails('Exile ~ with three time counters on them.');
  });

  it('offers either of a player\'s zones as the source of one selection', () => {
    assert.deepEqual(steps('You may put a land card from your hand or graveyard onto the battlefield tapped.'), [[[{
      effect: 'move-zone', what: { ref: 'filter', filter: { types: ['land'], cls: 'card', anyOf: [{ zone: yourHand }, { zone: yourGraveyard }] }, count: n(1) },
      to: { zone: 'battlefield' }, tapped: true, optional: true,
    }]]]);
    fails('You may put a land card from your hand or hand onto the battlefield.');
    fails('You may put a land card from your hand or graveyard or library onto the battlefield.');
  });

  it('treats the cards from a hand as that hand\'s complete contents', () => {
    const hand = { ref: 'zone-cards', zone: 'hand', owner: you };
    const expected = [[[{ effect: 'shuffle-into-library', who: you, what: [hand], library: yourLibrary }, { effect: 'draw', who: you, amount: { amount: 'that-much' } }]]];
    assert.deepEqual(steps('Shuffle the cards from your hand into your library, then draw that many cards.'), expected);
    assert.deepEqual(steps('Shuffle your hand into your library, then draw that many cards.'), expected);
    fails('Shuffle the cards from the battlefield into your library.');
  });

  it('marks cards outside the game without inventing a zone', () => {
    assert.deepEqual(steps('You may reveal a sorcery card you own from outside the game and put it into your hand.'), [[[{ effect: 'optional-group', who: you, steps: [
      { effect: 'reveal', who: you, what: { ref: 'filter', filter: { types: ['sorcery'], cls: 'card', own: 'you', outsideGame: true }, count: n(1) } },
      { effect: 'move-zone', what: { ref: 'it' }, to: yourHand },
    ] }]]]);
    fails('You may reveal a sorcery card you own from outside the battlefield and put it into your hand.');
    fails('Search outside the game for a card.');
  });
});

describe('pregame rules', () => {
  it('parses the leyline opening-hand action as a static permission', () => {
    assert.deepEqual(abilities('If this card is in your opening hand, you may begin the game with it on the battlefield.', 'Enchantment'), [{
      kind: 'static', effect: { static: 'opening-hand-action', what: { ref: 'this', noun: 'card' }, who: you, action: 'begin-on-battlefield' },
    }]);
  });

  it('rejects opening-hand wordings with extra conditions or other actions', () => {
    fails("If this card is in your opening hand and you're not the starting player, you may begin the game with it on the battlefield.", 'Land');
    fails('If this card is in your hand, you may begin the game with it on the battlefield.', 'Enchantment');
    fails('If this card is in your opening hand, you may begin the game with it on the battlefield with a luck counter on it.', 'Land');
    fails('If this card is in your opening hand, begin the game with it on the battlefield.', 'Enchantment');
  });

  it('records named deck-copy limits with and without a maximum', () => {
    assert.deepEqual(abilities('A deck can have any number of cards named "Relentless Rats".', 'Creature — Rat'), [{
      kind: 'static', effect: { static: 'deck-copy-limit', name: { name: 'literal', value: 'Relentless Rats' }, during: 'deck-construction' },
    }]);
    assert.deepEqual(abilities('A deck can have up to seven cards named "Seven Dwarves".', 'Creature — Dwarf'), [{
      kind: 'static', effect: { static: 'deck-copy-limit', name: { name: 'literal', value: 'Seven Dwarves' }, maximum: 7, during: 'deck-construction' },
    }]);
    fails('A deck can have any number of cards named "Relentless Rats" and any basic land cards.', 'Creature — Rat');
    fails('A deck can have any number of cards.', 'Creature — Rat');
  });
});
