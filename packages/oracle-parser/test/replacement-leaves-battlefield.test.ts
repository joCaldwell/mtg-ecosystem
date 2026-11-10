import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// "If it would leave the battlefield, exile it instead of putting it anywhere
// else" (CR 614.1a, 614.6), checked against the frozen Scryfall corpus: Whip
// of Erebos, Gruesome Encore, Kheru Lich Lord, From the Catacombs, Geth,
// Thane of Contracts, Realmbreaker, the Invasion Tree.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed' as const, value });
const exileIt = { replacement: 'effects', effects: [{ steps: [{ effect: 'exile', what: { ref: 'replacement-event-object', role: 'affected' } }] }] };
const leaves = (what: object) => ({ event: { event: 'move-zone', what, from: { zone: 'battlefield' } }, occurrence: 'each', replace: exileIt });
function abilities(text: string, typeLine = 'Sorcery') {
  const parsed = parseOracleText(text, 'Test Card', { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities)) as unknown[];
}
const fails = (text: string, typeLine = 'Sorcery') => assert.equal(parseOracleText(text, 'Test Card', { typeLine }).ok, false, text);

describe('leaves-the-battlefield replacement with "instead of putting it anywhere else"', () => {
  it('is the same node as the bare "exile it instead"', () => {
    const long = abilities('If this creature would leave the battlefield, exile it instead of putting it anywhere else.', 'Creature — Zombie');
    assert.deepEqual(long, [{ kind: 'static', effect: { static: 'replacement', replacement: leaves({ ref: 'this', noun: 'creature' }) } }]);
    assert.deepEqual(long, abilities('If this creature would leave the battlefield, exile it instead.', 'Creature — Zombie'));
  });
  it('is granted as a quoted static ability (Geth, Thane of Contracts)', () => {
    const card = { ref: 'target', filter: { types: ['creature'], cls: 'card', zone: { zone: 'graveyard', owner: 'your' } }, count: n(1) };
    assert.deepEqual(abilities('Return target creature card from your graveyard to the battlefield. It gains "If this creature would leave the battlefield, exile it instead of putting it anywhere else."'), [{
      kind: 'spell', effects: [
        { steps: [{ effect: 'move-zone', what: card, to: { zone: 'battlefield' } }] },
        { steps: [{ effect: 'gain-abilities', what: { ref: 'it' }, abilities: [{ gained: 'quoted', ability: {
          kind: 'static', effect: { static: 'replacement', replacement: leaves({ ref: 'this', noun: 'creature' }) } } }] }] }] }]);
  });
  it('follows a delayed exile as its own instruction (Whip of Erebos, Gruesome Encore)', () => {
    const [ability] = abilities('Return target creature card from your graveyard to the battlefield. Exile it at the beginning of the next end step. If it would leave the battlefield, exile it instead of putting it anywhere else.') as { effects: unknown[] }[];
    assert.deepEqual(ability.effects.slice(1), [
      { steps: [{ effect: 'create-delayed-trigger', trigger: { trigger: 'phase', phase: 'end-step', whose: 'each-player' }, occurrence: 'next', nextOnly: true,
        effects: [{ steps: [{ effect: 'exile', what: { ref: 'it' } }] }], targets: 'enclosing-ability' }] },
      { steps: [{ effect: 'create-replacement', replacement: leaves({ ref: 'it' }) }] }]);
    const [encore] = abilities('Return target creature card from your graveyard to the battlefield. Exile it at the beginning of the next end step. If that creature would leave the battlefield, exile it instead of putting it anywhere else.') as { effects: unknown[] }[];
    assert.deepEqual(encore.effects[2], { steps: [{ effect: 'create-replacement', replacement: leaves({ ref: 'that', filter: { types: ['creature'] } }) }] });
  });
  it('rejects the tail on other events and other printed tails', () => {
    // Dying already names its destination; "anywhere else" has nothing to restate.
    fails('If this creature would die, exile it instead of putting it anywhere else.', 'Creature — Zombie');
    fails('If this creature would leave the battlefield, exile it instead of putting it into your graveyard.', 'Creature — Zombie');
    fails('If this creature would leave the battlefield, exile it instead of putting them anywhere.', 'Creature — Zombie');
    // An ordinary condition after a delayed instruction keeps its ambiguous scope.
    fails('Return target creature card from your graveyard to the battlefield. Exile it at the beginning of the next end step. If you control a Zombie, draw a card.');
    fails('Return target creature card from your graveyard to the battlefield. Exile it at the beginning of the next end step. When it would leave the battlefield, draw a card.');
  });
});
