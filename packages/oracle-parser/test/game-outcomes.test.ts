import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// Card text checked against the frozen Scryfall corpus. Winning/losing is an
// explicit game outcome (CR 104.2b, 104.3e), not a change in life total.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed', value });
const you = { player: 'you' };
function abilities(text: string, typeLine?: string) {
  const r = parseOracleText(text, undefined, { typeLine, includeSourceProvenance: true });
  assert.ok(r.ok, JSON.stringify(r.lines));
  return JSON.parse(JSON.stringify(r.abilities));
}

describe('explicit game outcomes and their full conditions', () => {
  it('keeps Battle of Wits and Mortal Combat intervening checks on current zone contents', () => {
    for (const [text, of, count] of [
      ['you have 200 or more cards in your library', { cls: 'card', zone: { zone: 'library', owner: you } }, 200],
      ['twenty or more creature cards are in your graveyard', { types: ['creature'], cls: 'card', zone: { zone: 'graveyard', owner: 'your' } }, 20],
    ] as const) {
      assert.deepEqual(abilities(`At the beginning of your upkeep, if ${text}, you win the game.`, 'Enchantment'), [{
        kind: 'triggered', trigger: { trigger: 'phase', phase: 'upkeep', whose: 'your' },
        condition: { condition: 'count', of, comparison: { op: 'ge', value: n(count) } },
        effects: [{ steps: [{ effect: 'win-game', who: you }] }],
      }]);
    }
  });

  it('retains every Door to Nothingness activation cost and the targeted loser', () => {
    assert.deepEqual(abilities('This artifact enters tapped.\n{W}{W}{U}{U}{B}{B}{R}{R}{G}{G}, {T}, Sacrifice this artifact: Target player loses the game.', 'Artifact'), [
      { kind: 'static', effect: { static: 'enters-tapped', what: { ref: 'this', noun: 'artifact' } } },
      { kind: 'activated', costs: [
        { cost: 'mana', symbols: ['W', 'W', 'U', 'U', 'B', 'B', 'R', 'R', 'G', 'G'] },
        { cost: 'tap-self' }, { cost: 'sacrifice', what: { ref: 'this', noun: 'artifact' } },
      ], effects: [{ steps: [{ effect: 'lose-game', who: { player: 'target-player', selectionId: 'line:1:17' } }] }] },
    ]);
  });

  it('preserves every Demonic Pact mode and the permanent lifetime of its choice history', () => {
    assert.deepEqual(abilities("At the beginning of your upkeep, choose one that hasn't been chosen —\n• This enchantment deals 4 damage to any target and you gain 4 life.\n• Target opponent discards two cards.\n• Draw two cards.\n• You lose the game.", 'Enchantment'), [{
      kind: 'triggered', trigger: { trigger: 'phase', phase: 'upkeep', whose: 'your' }, effects: [{ steps: [{
        effect: 'modal', count: { exactly: 1 }, unchosen: { period: 'source-object' }, options: [
          { effects: [{ steps: [
            { effect: 'damage', source: { ref: 'this', noun: 'enchantment' }, amount: n(4), to: { to: 'any-target', selectionId: 'line:1:8' } },
            { effect: 'gain-life', who: you, amount: n(4) },
          ] }] },
          { effects: [{ steps: [{ effect: 'discard', who: { player: 'target-opponent', selectionId: 'line:2:1' }, what: { ref: 'filter', filter: { cls: 'card' }, count: n(2) } }] }] },
          { effects: [{ steps: [{ effect: 'draw', who: you, amount: n(2) }] }] },
          { effects: [{ steps: [{ effect: 'lose-game', who: you }] }] },
        ],
      }] }],
    }]);
  });

  it('does not confuse life loss with losing the game or discard unsupported outcome qualifications', () => {
    assert.deepEqual(abilities('You lose 2 life. You win the game.'), [{ kind: 'spell', effects: [
      { steps: [{ effect: 'lose-life', who: you, amount: n(2) }] },
      { steps: [{ effect: 'win-game', who: you }] },
    ] }]);
    for (const text of ['You win the next game.', 'You lose the game until end of turn.', 'You win the game unless you pay a banana.', 'You win.']) {
      assert.equal(parseOracleText(text).ok, false, text);
    }
  });
});
