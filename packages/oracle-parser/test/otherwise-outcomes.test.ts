import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';
import { buildCardNameCatalog } from '../src/card-names.ts';

// Source text: frozen Scryfall corpus. Optional-placement outcomes verified in
// https://magic.wizards.com/en/news/feature/oath-gatewatch-release-notes-2016-01-13
// https://magic.wizards.com/en/news/feature/aether-revolt-release-notes-2017-01-06
const n = (value: number) => ({ amount: 'fixed', value });
const you = { player: 'you' };
const itRef = { ref: 'it' };
const thatCard = { ref: 'that', filter: { cls: 'card' } };
const topCard = { ref: 'library-cards', position: 'top', count: n(1), of: { zone: 'library', owner: 'your' } };
const reveal = { steps: [{ effect: 'reveal', who: you, what: topCard }] };
function abilities(text: string, typeLine?: string) {
  const r = parseOracleText(text, undefined, { typeLine, cardNames: buildCardNameCatalog([{ name: 'Wastes', layout: 'normal' }]) });
  assert.ok(r.ok, JSON.stringify(r.lines));
  return JSON.parse(JSON.stringify(r.abilities));
}

describe('otherwise branches preserve the tested outcome', () => {
  it('keeps Matter Reshaper hand placement when eligibility fails, placement is declined or cannot happen', () => {
    assert.deepEqual(abilities("When this creature dies, reveal the top card of your library. You may put that card onto the battlefield if it's a permanent card with mana value 3 or less. Otherwise, put that card into your hand.", 'Creature — Eldrazi'), [{
      kind: 'triggered', trigger: { trigger: 'dies', what: { ref: 'this', noun: 'creature' } }, effects: [reveal, {
        condition: { condition: 'is-filter', what: itRef, filter: { cls: 'card', permanent: true, properties: [{ property: 'mana-value', comparison: { op: 'le', value: n(3) } }] } },
        conditionKind: 'if', conditionPosition: 'after', steps: [{ effect: 'move-zone', what: thatCard, to: { zone: 'battlefield' }, optional: true }],
        otherwise: { on: 'action-not-performed', steps: [{ effect: 'move-zone', what: thatCard, to: { zone: 'hand', owner: 'your' } }] },
      }],
    }]);
  });
  it('preserves Aid from the Cowl intervening condition and independently optional bottom placement', () => {
    assert.deepEqual(abilities("Revolt — At the beginning of your end step, if a permanent left the battlefield under your control this turn, reveal the top card of your library. If it's a permanent card, you may put it onto the battlefield. Otherwise, you may put it on the bottom of your library.", 'Enchantment'), [{
      kind: 'triggered', abilityWord: 'Revolt', trigger: { trigger: 'phase', phase: 'end-step', whose: 'your' },
      condition: { condition: 'zone-change-history', event: 'leaves-battlefield', subject: { match: 'filter', filter: { cls: 'permanent' }, comparison: { op: 'ge', value: n(1) } }, from: { zone: 'battlefield' }, controllerAtMove: you, during: 'this-turn' },
      effects: [reveal, {
        condition: { condition: 'is-filter', what: itRef, filter: { cls: 'card', permanent: true } }, conditionKind: 'if',
        steps: [{ effect: 'move-zone', what: itRef, to: { zone: 'battlefield' }, optional: true }],
        otherwise: { on: 'action-not-performed', steps: [{ effect: 'move-zone', what: itRef, to: { zone: 'library', owner: 'your', position: 'bottom' }, optional: true }] },
      }],
    }]);
  });
  it('keeps Ruin in Their Wake tapped placement, board condition, hand fallback and unconditional shuffle', () => {
    assert.deepEqual(abilities("Search your library for a basic land card and reveal it. You may put that card onto the battlefield tapped if you control a land named Wastes. Otherwise, put that card into your hand. Then shuffle."), [{
      kind: 'spell', effects: [
        { steps: [
          { effect: 'search', who: you, zones: { selection: 'all', of: [{ zone: 'library', owner: 'your' }] }, for: { ref: 'filter', filter: { supertypes: ['basic'], types: ['land'], cls: 'card' }, count: n(1) } },
          { effect: 'reveal', who: you, what: itRef },
        ] },
        { condition: { condition: 'control', who: you, what: { ref: 'filter', filter: { types: ['land'], name: { name: 'literal', value: 'Wastes' } }, count: n(1) } }, conditionKind: 'if', conditionPosition: 'after',
          steps: [{ effect: 'move-zone', what: thatCard, to: { zone: 'battlefield' }, tapped: true, optional: true }],
          otherwise: { on: 'action-not-performed', steps: [{ effect: 'move-zone', what: thatCard, to: { zone: 'hand', owner: 'your' } }] },
        },
        { steps: [{ effect: 'shuffle', who: you, library: { zone: 'library', owner: you } }] },
      ],
    }]);
  });
  it('distinguishes IF and UNLESS condition outcomes from optional action outcomes', () => {
    const draw = { effect: 'draw', who: you, amount: n(1) };
    assert.deepEqual(abilities('If you have 10 life, draw a card. Otherwise, you gain 2 life.'), [{ kind: 'spell', effects: [{
      condition: { condition: 'life-total', who: you, comparison: { op: 'eq', value: n(10) } }, conditionKind: 'if', steps: [draw],
      otherwise: { on: 'condition-false', steps: [{ effect: 'gain-life', who: you, amount: n(2) }] },
    }] }]);
    assert.deepEqual(abilities('Destroy target creature unless its controller pays {2}. Otherwise, draw a card.'), [{ kind: 'spell', effects: [{
      condition: { condition: 'pays', who: { player: 'controller', of: itRef }, cost: [{ cost: 'mana', symbols: ['2'] }] }, conditionKind: 'unless',
      steps: [{ effect: 'destroy', what: { ref: 'target', filter: { types: ['creature'] }, count: n(1) } }],
      otherwise: { on: 'condition-true', steps: [draw] },
    }] }]);
    for (const text of [
      'If you have 10 life, you may draw a card. Otherwise, you gain 2 life.',
      "If it's a creature card, you may reveal it and put it into your hand. Otherwise, put it into your graveyard.",
      "If it's a creature card, you may put it onto the battlefield and draw a card. Otherwise, put it into your hand.",
      "If it's a creature card, you may put it onto the battlefield. Otherwise, put target creature into your hand.",
      'You may put it onto the battlefield. Otherwise, put it into your hand.',
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
});
