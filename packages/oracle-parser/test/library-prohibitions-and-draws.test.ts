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
const players = { player: 'each-player' };
const rule = (who: object, body: object) => ({ static: 'player-rule', rule: { who, ...body } });

describe('player prohibitions on drawing, searching and cycling (CR 121.3, 701.23, 702.29)', () => {
  it('states the prohibited action as a player rule', () => {
    assert.deepEqual(abilities('Players can\'t draw cards.', 'Artifact'), [{ kind: 'static', effect: rule(players, { rule: 'cant-draw-cards' }) }]);
    assert.deepEqual(abilities('Players can\'t cycle cards.', 'Artifact'), [{ kind: 'static', effect: rule(players, { rule: 'cant-cycle-cards' }) }]);
    assert.deepEqual(abilities('Your opponents can\'t search libraries.', 'Enchantment'),
      [{ kind: 'static', effect: rule({ player: 'each-opponent' }, { rule: 'cant-search-libraries' }) }]);
  });

  it('applies the negation to every coordinated action', () => {
    assert.deepEqual(abilities('Players can\'t draw cards or gain life.', 'Enchantment'), [{ kind: 'static', effect: { static: 'group', effects: [
      rule(players, { rule: 'cant-draw-cards' }), rule(players, { rule: 'cant-gain-life' })] } }]);
  });

  it('rejects near misses', () => {
    fails('Players can\'t draw.', 'Artifact');
    fails('Players can\'t draw cards or win.', 'Artifact');
  });
});

describe('draw amounts: additional and up to (CR 121.2, 504.1)', () => {
  it('treats "additional" as an ordinary draw beside the turn-based one', () => {
    assert.deepEqual(abilities('At the beginning of your draw step, draw two additional cards.', 'Enchantment')[0].effects,
      [{ steps: [{ effect: 'draw', who: { player: 'you' }, amount: { amount: 'fixed', value: 2 } }] }]);
  });

  it('lets the drawing player choose a number from zero to the stated maximum', () => {
    assert.deepEqual(abilities('Each player draws up to seven cards.')[0].effects[0].steps, [
      { effect: 'draw', who: { player: 'each-player' }, amount: { amount: 'chosen', minimum: 0, maximum: { amount: 'fixed', value: 7 } } }]);
    assert.deepEqual(abilities('Draw up to X cards.')[0].effects[0].steps[0].amount, { amount: 'chosen', minimum: 0, maximum: { amount: 'x' } });
  });

  it('rejects "additional" without a number and "up to" without an amount', () => {
    fails('Draw additional cards.');
    fails('Draw up to cards.');
  });
});

describe('revealing a card from the opening hand (CR 103.6b)', () => {
  const thisCard = { ref: 'this', noun: 'card' };
  const reveal = (effects: object[]) => [{ kind: 'static', effect: { static: 'opening-hand-action', what: thisCard, who: { player: 'you' }, action: 'reveal', effects } }];

  it('keeps the "if you do" instructions as a delayed trigger on the first turn', () => {
    assert.deepEqual(abilities('You may reveal this card from your opening hand. If you do, at the beginning of the first upkeep, each opponent mills seven cards.', 'Creature — Phyrexian'),
      reveal([{ steps: [{ effect: 'create-delayed-trigger',
        trigger: { trigger: 'phase-in-turn', phase: 'upkeep', turn: { turn: 'first' } }, occurrence: 'next', nextOnly: true, targets: 'delayed-ability',
        effects: [{ steps: [{ effect: 'mill', who: { player: 'each-opponent' }, amount: { amount: 'fixed', value: 7 } }] }] }] }]));
  });

  it('distinguishes the first turn of the game from your own first turn', () => {
    const trigger = (text: string) => abilities(text, 'Creature — Phyrexian')[0].effect.effects[0].steps[0].trigger;
    assert.deepEqual(trigger('You may reveal this card from your opening hand. If you do, scry 3 at the beginning of your first upkeep.'),
      { trigger: 'phase-in-turn', phase: 'upkeep', turn: { turn: 'first', whose: { player: 'you' } } });
    assert.deepEqual(trigger('You may reveal this card from your opening hand. If you do, at the beginning of your first main phase of the game, add {G}.'),
      { trigger: 'phase-in-turn', phase: 'precombat-main', turn: { turn: 'first', whose: { player: 'you' } } });
  });

  it('does not accept first-turn wording outside a pregame action', () => {
    fails('At the beginning of the first upkeep, each opponent mills seven cards.');
    fails('At the beginning of your first upkeep, you lose the game.');
    fails('You may reveal this card from your hand. If you do, at the beginning of the first upkeep, each opponent mills seven cards.', 'Creature — Phyrexian');
    fails('You may reveal this card from your opening hand. At the beginning of the first upkeep, each opponent mills seven cards.', 'Creature — Phyrexian');
  });
});
