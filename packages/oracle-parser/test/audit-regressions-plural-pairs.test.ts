// Regressions found by independent audits of new parses.
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';

describe('audit fixes: plural pairs and shared graveyards', () => {
  it('does not carry a leading modifier to the second noun of a bare plural pair (Cid, Timeless Artificer)', () => {
    // "Artifact creatures and Heroes": a Hero need not be an artifact; the reach is not stated.
    assert.equal(parseOracleText('Artifact creatures and Heroes you control get +1/+1.', 'Fixture', { typeLine: 'Enchantment' }).ok, false);
    assert.equal(parseOracleText('Other Treefolk and Forests you control have indestructible.', 'Fixture', { typeLine: 'Creature — Treefolk' }).ok, true);
  });
  it('takes several targets from one player\'s graveyard (Lodestone Bauble)', () => {
    const result = parseOracleText("Put up to four target basic land cards from a player's graveyard on top of their library in any order.", 'Fixture', { typeLine: 'Sorcery' });
    assert.match(JSON.stringify(result), /"zone":\{"zone":"graveyard","owner":"any-single"\}/);
  });
});
