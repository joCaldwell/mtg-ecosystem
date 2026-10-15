import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';
import { buildCardNameCatalog } from '../src/card-names.ts';

// Oracle wordings checked against the frozen corpus; rules from the
// Comprehensive Rules of 2026-09-25.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed', value });
const you = { player: 'you' };
const yourHand = { zone: 'hand', owner: 'your' };
const yourGraveyard = { zone: 'graveyard', owner: 'your' };
const cardNames = buildCardNameCatalog([{ name: 'Stomping Slabs', layout: 'normal' }]);
function abilities(text: string, typeLine = 'Sorcery') {
  const r = parseOracleText(text, undefined, { typeLine, cardNames });
  assert.ok(r.ok, JSON.stringify(r.lines));
  return JSON.parse(JSON.stringify(r.abilities));
}
const fails = (text: string, typeLine = 'Sorcery') => assert.equal(parseOracleText(text, undefined, { typeLine, cardNames }).ok, false, text);

describe('cast origin stated by exclusion (CR 601.2a)', () => {
  it('records the excluded origin of a cast trigger, never a current zone', () => {
    assert.deepEqual(abilities('Whenever you cast a spell from anywhere other than your hand, draw a card.', 'Enchantment'), [{
      kind: 'triggered', trigger: { trigger: 'cast', who: you, what: { cls: 'spell' }, notFrom: yourHand },
      effects: [{ steps: [{ effect: 'draw', who: you, amount: n(1) }] }],
    }]);
    assert.deepEqual(abilities('Whenever you cast a spell from your graveyard, draw a card.', 'Enchantment')[0].trigger,
      { trigger: 'cast', who: you, what: { cls: 'spell' }, from: yourGraveyard });
  });

  it('records the excluded origin of a was-cast condition', () => {
    assert.deepEqual(abilities('Draw a card. If this spell was cast from anywhere other than your hand, draw two cards.')[0].effects[1], {
      condition: { condition: 'was-cast', what: { ref: 'this', noun: 'spell' }, notFrom: yourHand }, conditionKind: 'if',
      steps: [{ effect: 'draw', who: you, amount: n(2) }],
    });
  });

  it('rejects one trailing exclusion after alternatives without their own origin', () => {
    fails('Whenever you play a land or cast a spell from anywhere other than your hand, draw a card.', 'Enchantment');
    fails('Whenever you cast a spell from anywhere, draw a card.', 'Enchantment');
  });
});

describe('distinct mana values among cards (CR 202.3)', () => {
  const graveyardCards = { ref: 'each', filter: { cls: 'card', zone: yourGraveyard } };
  const distinct = { amount: 'distinct-count', characteristic: 'mana-value', among: graveyardCards };

  it('compares the number of different mana values, not the number of cards', () => {
    assert.deepEqual(abilities('Draw two cards. Then discard a card unless there are five or more mana values among cards in your graveyard.')[0].effects[1], {
      condition: { condition: 'compare', value: distinct, comparison: { op: 'ge', value: n(5) } }, conditionKind: 'unless',
      steps: [{ effect: 'discard', who: you, what: { ref: 'filter', filter: { cls: 'card' }, count: n(1) } }],
    });
  });

  it('scales one draw by the distinct values and binds X to them', () => {
    assert.deepEqual(abilities('Draw a card for each different mana value among cards in your graveyard.')[0].effects, [{
      steps: [{ effect: 'draw', who: you, amount: { amount: 'product', factors: [n(1), distinct] } }],
    }]);
    assert.deepEqual(abilities('Draw X cards, where X is the number of different mana values among cards in your graveyard.')[0].effects[0].bindings,
      [{ variable: 'x', value: distinct }]);
  });

  it('keeps the greatest mana value a different amount and rejects number disagreement', () => {
    assert.deepEqual(abilities('Draw X cards, where X is the greatest mana value among cards in your graveyard.')[0].effects[0].bindings, [{
      variable: 'x', value: { amount: 'attribute-extremum', extremum: 'maximum', attribute: 'mana-value', among: { cls: 'card', zone: yourGraveyard } },
    }]);
    fails('Draw a card for each different mana values among cards in your graveyard.');
    fails('Draw X cards, where X is the number of mana value among cards in your graveyard.');
    fails('Draw X cards, where X is the number of mana values in your graveyard.');
  });
});

describe('objects identified by a completed zone change (CR 400.7, 608.2h)', () => {
  it('counts moves into a zone from a printed origin during this turn', () => {
    assert.deepEqual(abilities('Draw a card for each creature put into your graveyard from the battlefield this turn.')[0].effects, [{
      steps: [{ effect: 'draw', who: you, amount: n(1), forEach: { types: ['creature'], relation: {
        relation: 'zone-change', to: yourGraveyard, from: { zone: 'battlefield' }, period: 'this-turn' } } }],
    }]);
    assert.deepEqual(abilities('You gain life equal to the number of cards put into your graveyard from anywhere this turn.')[0].effects[0].steps[0].amount, {
      amount: 'count', of: { cls: 'card', relation: { relation: 'zone-change', to: yourGraveyard, from: 'anywhere', period: 'this-turn' } },
    });
  });

  it('keeps the destination of moves made this way, for every moving verb', () => {
    const relation = (to: unknown) => ({ cls: 'card', relation: { relation: 'zone-change', to, scope: 'this-way' } });
    const count = (text: string) => abilities(text)[0].effects.at(-1).steps[0].amount;
    assert.deepEqual(count('Mill three cards. You gain life equal to the number of cards put into your graveyard this way.'),
      { amount: 'count', of: relation(yourGraveyard) });
    assert.deepEqual(count('Return all creatures to their owners\' hands. You gain life equal to the number of cards returned to your hand this way.'),
      { amount: 'count', of: relation(yourHand) });
    assert.deepEqual(count('Shuffle your graveyard into your library. You gain life equal to the number of cards shuffled into your library this way.'),
      { amount: 'count', of: relation({ zone: 'library', owner: 'your' }) });
  });

  it('keeps the unlinked result relation and rejects a move without a window or zone owner', () => {
    assert.deepEqual(abilities('Mill three cards. You gain life equal to the number of cards milled this way.')[0].effects[1].steps[0].amount,
      { amount: 'count', of: { cls: 'card', relation: { relation: 'event-result', event: 'mill', scope: 'this-way' } } });
    fails('You gain life equal to the number of cards put into your graveyard.');
    fails('Mill three cards. You gain life equal to the number of cards put into graveyards this way.');
    fails('You gain life equal to the number of cards put into the top of your library this way.');
  });
});

describe('counts stated after the noun (CR 107.1, 608.2h)', () => {
  const swamps = { amount: 'count', of: { subtypes: ['Swamp'], control: 'you' } };
  const target = { player: 'target-player' };

  it('reads "a number of cards from their hand equal to N" as N chosen cards of that hand', () => {
    assert.deepEqual(abilities('Target player reveals a number of cards from their hand equal to the number of Swamps you control.')[0].effects, [{
      steps: [{ effect: 'reveal', who: target, what: { ref: 'filter', filter: { cls: 'card', zone: { zone: 'hand', owner: 'their' } }, count: swamps } }],
    }]);
    assert.deepEqual(abilities('Target player discards a number of cards equal to the number of Swamps you control.')[0].effects[0].steps,
      [{ effect: 'discard', who: target, what: { ref: 'filter', filter: { cls: 'card' }, count: swamps } }]);
  });

  it('reads the same count from the top of a library as its top N cards, with no choice', () => {
    assert.deepEqual(abilities('Reveal a number of cards from the top of your library equal to the number of Swamps you control.')[0].effects[0].steps, [{
      effect: 'reveal', who: you, what: { ref: 'library-cards', position: 'top', count: swamps, of: { zone: 'library', owner: 'your' } },
    }]);
  });

  it('reads "discard cards equal to N" like the counted form and rejects a missing or filtered top-of-library count', () => {
    assert.deepEqual(abilities('Draw two cards, then discard cards equal to the number of Swamps you control.')[0].effects[0].steps[1],
      { effect: 'discard', who: you, what: { ref: 'filter', filter: { cls: 'card' }, count: swamps } });
    fails('Target player reveals a number of cards from their hand.');
    fails('Reveal a number of creature cards from the top of your library equal to the number of Swamps you control.');
    fails('Reveal a number of cards from the top of your library.');
  });
});

describe('one closing zone for coordinated target cards (CR 109.2a)', () => {
  const target = (types: string[], extra: object = {}) => ({ ref: 'target', filter: { types, cls: 'card', zone: yourGraveyard }, count: n(1), ...extra });

  it('gives each separately targeted card the zone that closes the phrase', () => {
    assert.deepEqual(abilities('Return up to one target instant card and up to one target sorcery card from your graveyard to your hand.')[0].effects, [{
      steps: [{ effect: 'simultaneous', steps: [
        { effect: 'return-to-hand', what: target(['instant'], { upTo: true }), to: 'your' },
        { effect: 'return-to-hand', what: target(['sorcery'], { upTo: true }), to: 'your' },
      ] }],
    }]);
    assert.deepEqual(abilities('Exile target creature card and target land card from your graveyard.')[0].effects[0].steps, [{
      effect: 'simultaneous', steps: [{ effect: 'exile', what: target(['creature']) }, { effect: 'exile', what: target(['land']) }],
    }]);
  });

  it('does not share an inner zone qualifier or scope untargeted selections', () => {
    fails('Return target instant card and target sorcery card from your graveyard with mana value 3 to your hand.');
    fails('Return target instant card and a sorcery card from your graveyard to your hand.');
    fails('Return a creature card and target sorcery card from your graveyard to your hand.');
  });
});

describe('cards by position in a graveyard (CR 404.1–404.2)', () => {
  const top = (extra: object = {}, of: object = yourGraveyard) => ({ ref: 'graveyard-cards', position: 'top', count: n(1), ...extra, of });

  it('takes the uppermost matching card, which is not a choice', () => {
    assert.deepEqual(abilities('Return the top creature card of your graveyard to the battlefield.')[0].effects, [{
      steps: [{ effect: 'move-zone', what: top({ matching: { types: ['creature'] } }), to: { zone: 'battlefield' } }],
    }]);
    assert.deepEqual(abilities('Return a creature card from your graveyard to the battlefield.')[0].effects[0].steps[0].what,
      { ref: 'filter', filter: { types: ['creature'], cls: 'card', zone: yourGraveyard }, count: n(1) });
  });

  it('takes the top cards themselves when no description is printed', () => {
    assert.deepEqual(abilities('Put the top card of your graveyard on the bottom of your library.')[0].effects[0].steps, [{
      effect: 'move-zone', what: top(), to: { zone: 'library', owner: 'your', position: 'bottom' },
    }]);
    assert.deepEqual(abilities('Exile the top two cards of target player\'s graveyard.')[0].effects[0].steps, [{
      effect: 'exile', what: { ref: 'graveyard-cards', position: 'top', count: n(2), of: { zone: 'graveyard', owner: { player: 'target-player' } } },
    }]);
  });

  it('rejects pooled graveyards, number disagreement and the bottom of a graveyard', () => {
    fails('Exile the top card of all graveyards.');
    fails('Exile the top card of each graveyard.');
    fails('Exile the top two card of your graveyard.');
    fails('Exile the top cards of your graveyard.');
    fails('Exile the bottom card of your graveyard.');
    fails('Exile the top creature card you own of your graveyard.');
  });
});

describe('several personal zones under "all" (CR 400.1)', () => {
  const both = (owner: unknown, extra: object = {}) => ({ ref: 'all', filter: { ...extra, cls: 'card', anyOf: [{ zone: { zone: 'hand', owner } }, { zone: { zone: 'graveyard', owner } }] } });

  it('extends "all cards from X\'s hand and graveyard" to the cards of both zones', () => {
    assert.deepEqual(abilities('Exile all cards from target player\'s hand and graveyard.')[0].effects, [{
      steps: [{ effect: 'exile', what: both({ player: 'target-player' }) }],
    }]);
    assert.deepEqual(abilities('Exile all noncreature, nonland cards from your hand and graveyard.')[0].effects[0].steps[0].what,
      { ref: 'all', filter: { nonTypes: ['creature', 'land'], cls: 'card', anyOf: [{ zone: yourHand }, { zone: yourGraveyard }] } });
  });

  it('pools every player\'s or every opponent\'s zones when the quantifier is plural', () => {
    assert.deepEqual(abilities('Exile all cards from all hands and graveyards.')[0].effects[0].steps[0].what, both('all-players'));
    assert.deepEqual(abilities('Exile all cards from all opponents\' hands and graveyards.')[0].effects[0].steps[0].what, both('all-opponents'));
    assert.deepEqual(abilities('Exile all opponents\' graveyards.')[0].effects[0].steps,
      [{ effect: 'exile', what: { ref: 'zone-cards', zone: 'graveyard', owner: { player: 'each-opponent' } } }]);
  });

  it('rejects number disagreement, a repeated zone and a counted selection across two zones', () => {
    fails('Exile all cards from all hands and graveyard.');
    fails('Exile all cards from your hand and graveyards.');
    fails('Exile all cards from your hand and hand.');
    fails('Exile a card from your hand and graveyard.');
    fails('Exile all cards from your hand, graveyard, and library.');
  });
});

describe('the milled cards as a definite event result (CR 701.17a)', () => {
  const milled = (number: string) => ({ ref: 'related', number, filter: { cls: 'card' }, relation: { relation: 'event-result', event: 'mill', scope: 'contextual' } });

  it('selects from among the cards the mill moved', () => {
    assert.deepEqual(abilities('Mill four cards. You may put a land card from among the milled cards into your hand.')[0].effects, [
      { steps: [{ effect: 'mill', who: you, amount: n(4) }] },
      { steps: [{ effect: 'move-zone', what: { ref: 'filter', filter: { types: ['land'], cls: 'card', fromAmong: milled('plural') }, count: n(1) }, to: yourHand, optional: true }] },
    ]);
  });

  it('refers to the single milled card and keeps "milled this way" as a filter relation', () => {
    assert.deepEqual(abilities('Mill a card. You gain X life, where X is the milled card\'s mana value.')[0].effects[1].bindings,
      [{ variable: 'x', value: { amount: 'attribute', of: milled('singular'), attribute: 'mana-value' } }]);
    assert.deepEqual(abilities('Mill four cards. You may put a land card milled this way into your hand.')[0].effects[1].steps[0].what,
      { ref: 'filter', filter: { types: ['land'], cls: 'card', relation: { relation: 'event-result', event: 'mill', scope: 'this-way' } }, count: n(1) });
  });

  it('gives the other this-way-only participles no bare definite form', () => {
    fails('Draw two cards. Exile the drawn cards.');
    fails('Mill four cards. Exile all milled cards.');
    fails('Mill four cards. Exile those milled cards.');
  });
});

describe('cards exiled from a stated zone this way (CR 400.7)', () => {
  it('records the move into exile with its printed origin', () => {
    assert.deepEqual(abilities('Target player exiles two cards from their hand, then draws a card for each card exiled from their hand this way.')[0].effects[0].steps[1], {
      effect: 'draw', who: { player: 'target-player' }, amount: n(1),
      forEach: { cls: 'card', relation: { relation: 'zone-change', to: { zone: 'exile' }, from: { zone: 'hand', owner: 'their' }, scope: 'this-way' } },
    });
  });

  it('keeps the plain event result without an origin and needs a window', () => {
    assert.deepEqual(abilities('Exile two cards from your hand, then draw a card for each card exiled this way.')[0].effects[0].steps[1].forEach,
      { cls: 'card', relation: { relation: 'event-result', event: 'exile', scope: 'this-way' } });
    fails('Exile two cards from your hand, then draw a card for each card exiled from your hand.');
  });
});

describe('piles (CR 700.3)', () => {
  const top5 = { effect: 'reveal', who: you, what: { ref: 'library-cards', position: 'top', count: n(5), of: { zone: 'library', owner: 'your' } } };
  const opponent = { player: 'an-opponent' };
  const rest = (to: unknown) => ({ effect: 'move-zone', what: { ref: 'rest' }, to });

  it('keeps who separates and lets the putting player pick a pile', () => {
    assert.deepEqual(abilities('Reveal the top five cards of your library. An opponent separates those cards into two piles. Put one pile into your hand and the other into your graveyard.')[0].effects, [
      { steps: [top5] },
      { steps: [{ effect: 'separate-piles', who: opponent, what: { ref: 'that', filter: { cls: 'card' }, plural: true }, piles: [{}, {}] }] },
      { steps: [{ effect: 'simultaneous', steps: [
        { effect: 'move-zone', what: { ref: 'pile', pile: 'choice', who: you }, to: yourHand }, rest(yourGraveyard)] }] },
    ]);
  });

  it('keeps who chooses when another player picks the pile', () => {
    assert.deepEqual(abilities('Reveal the top five cards of your library and separate them into two piles. An opponent chooses one of those piles. Put that pile into your hand and the other into your graveyard.')[0].effects, [
      { steps: [top5, { effect: 'separate-piles', who: you, what: { ref: 'them' }, piles: [{}, {}] }] },
      { steps: [{ effect: 'choose-pile', who: opponent }] },
      { steps: [{ effect: 'simultaneous', steps: [{ effect: 'move-zone', what: { ref: 'pile', pile: 'chosen' }, to: yourHand }, rest(yourGraveyard)] }] },
    ]);
  });

  it('records printed pile facings and the separating player of a coordinated verb', () => {
    const target = { player: 'target-opponent' };
    assert.deepEqual(abilities('Target opponent looks at the top four cards of your library and separates them into a face-down pile and a face-up pile. Put one pile into your hand and the other into your graveyard.')[0].effects[0].steps, [
      { effect: 'look-at', who: target, what: { top: n(4), of: { zone: 'library', owner: 'your' } } },
      { effect: 'separate-piles', who: target, what: { ref: 'them' }, piles: [{ facing: 'down' }, { facing: 'up' }] },
    ]);
  });

  it('reads "in the pile of their choice" as membership of a pile that player picks', () => {
    assert.deepEqual(abilities('Separate all permanents target player controls into two piles. That player sacrifices all permanents in the pile of their choice.')[0].effects, [
      { steps: [{ effect: 'separate-piles', who: you, what: { ref: 'all', filter: { cls: 'permanent', controlledBy: { player: 'target-player' } } }, piles: [{}, {}] }] },
      { steps: [{ effect: 'sacrifice', who: { player: 'that-player' }, what: { ref: 'all', filter: { cls: 'permanent', fromAmong: { ref: 'pile', pile: 'choice', who: { player: 'they' } } } } }] },
    ]);
  });

  it('rejects a pile without a separation, "that pile" without a choice, and other pile counts', () => {
    fails('Reveal the top five cards of your library. Put one pile into your hand and the other into your graveyard.');
    fails('Reveal the top five cards of your library and separate them into two piles. Put that pile into your hand and the other into your graveyard.');
    fails('Reveal the top five cards of your library. An opponent chooses one of those piles.');
    fails('Reveal the top five cards of your library and separate them into three piles. Put one pile into your hand and the other into your graveyard.');
    fails('Reveal the top five cards of your library and separate them into a face-up pile and a face-up pile. Put one pile into your hand and the other into your graveyard.');
  });
});
