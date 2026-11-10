import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// Skipping as a replacement (CR 614.10, 614.10b, 500.7, 500.11), checked
// against the frozen Scryfall corpus: Ugin's Nexus, Stranglehold, Time Vault,
// Fasting, Possessed Portal, Obstinate Familiar, Notion Thief, Plagiarize.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed' as const, value });
const you = { player: 'you' };
function abilities(text: string, typeLine = 'Artifact') {
  const parsed = parseOracleText(text, 'Test Card', { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities)) as unknown[];
}
const standing = (replacement: object) => [{ kind: 'static', effect: { static: 'replacement', replacement } }];
const fails = (text: string, typeLine = 'Artifact') => assert.equal(parseOracleText(text, 'Test Card', { typeLine }).ok, false, text);
const draw = (who: object) => ({ event: 'draw', who, quantity: { op: 'eq', value: n(1) } });

describe('skipping a turn or step that would begin', () => {
  it('skips extra turns only (Ugin\'s Nexus, Stranglehold)', () => {
    assert.deepEqual(abilities('If a player would begin an extra turn, that player skips that turn instead.'),
      standing({ event: { event: 'begin-time', who: { player: 'a-player' }, what: { kind: 'turn' }, extra: true }, occurrence: 'each', replace: { replacement: 'skip' } }));
    assert.deepEqual(abilities('If an opponent would begin an extra turn, that player skips that turn instead.', 'Enchantment'),
      standing({ event: { event: 'begin-time', who: { player: 'an-opponent' }, what: { kind: 'turn' }, extra: true }, occurrence: 'each', replace: { replacement: 'skip' } }));
  });
  it('keeps the action that follows an optional skip inside it (Time Vault, Fasting)', () => {
    assert.deepEqual(abilities('If you would begin your turn while this artifact is tapped, you may skip that turn instead. If you do, untap this artifact.'),
      standing({ event: { event: 'begin-time', who: you, what: { kind: 'turn' } }, occurrence: 'each',
        replace: { replacement: 'skip', after: [{ steps: [{ effect: 'untap', what: { ref: 'this', noun: 'artifact' } }] }] },
        condition: { condition: 'status', what: { ref: 'this', noun: 'artifact' }, status: 'tapped' }, optional: { who: you } }));
    assert.deepEqual(abilities('If you would begin your draw step, you may skip that step instead. If you do, you gain 2 life.', 'Enchantment'),
      standing({ event: { event: 'begin-time', who: you, what: { kind: 'step', step: 'draw' } }, occurrence: 'each',
        replace: { replacement: 'skip', after: [{ steps: [{ effect: 'gain-life', who: you, amount: n(2) }] }] }, optional: { who: you } }));
  });
  it('rejects a mismatched noun or player, and a follow-up to a mandatory skip', () => {
    fails('If a player would begin an extra turn, that player skips that step instead.');
    fails('If a player would begin an extra turn, you skip that turn instead.');
    fails('If you would begin your draw step, you may skip that turn instead.');
    fails('If a player would begin their turn, that player skips that turn instead.');
    fails('If you would begin your turn, you skip that turn instead. If you do, untap this artifact.');
    fails('If you would begin your turn, you may skip that turn instead. If you do, untap it.');
  });
});

describe('skipping a draw', () => {
  it('skips the draw, optionally (Possessed Portal, Obstinate Familiar)', () => {
    assert.deepEqual(abilities('If a player would draw a card, that player skips that draw instead.'),
      standing({ event: draw({ player: 'a-player' }), occurrence: 'each', replace: { replacement: 'skip' } }));
    assert.deepEqual(abilities('If you would draw a card, you may skip that draw instead.', 'Creature — Lizard'),
      standing({ event: draw(you), occurrence: 'each', replace: { replacement: 'skip' }, optional: { who: you } }));
  });
  it('reads "skips that draw and X" as replacing the draw with X (Notion Thief, Plagiarize)', () => {
    assert.deepEqual(abilities('Until end of turn, if target player would draw a card, instead that player skips that draw and you draw a card.', 'Instant'), [{ kind: 'spell', effects: [{ steps: [{
      effect: 'create-replacement', replacement: { event: draw({ player: 'target-player' }), occurrence: 'each',
        replace: { replacement: 'effects', effects: [{ steps: [{ effect: 'draw', who: you, amount: n(1) }] }] } }, duration: { duration: 'end-of-turn' } }] }] }]);
  });
  it('rejects another player skipping, and a follow-up action inside an optional draw skip', () => {
    fails('If a player would draw a card, you skip that draw instead.');
    fails('If you would draw a card, you may skip that draw and draw two cards instead.', 'Creature — Lizard');
  });
});
