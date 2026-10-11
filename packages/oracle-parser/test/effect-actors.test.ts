import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText, analyzeCardBindings } from '../src/index.ts';

describe('explicit effect actors remain reference candidates', () => {
  it('binds a later that-player to the actor of an exile, tap, untap, or return', () => {
    for (const instruction of [
      'Target opponent exiles target creature.',
      'Target player taps target creature.',
      'Target player untaps target creature.',
      "Target player returns target creature to its owner's hand.",
    ]) {
      const name = 'Actor fixture';
      const result = parseOracleText(`${instruction} That player draws a card.`, name, { includeSourceProvenance: true });
      assert.ok(result.ok, JSON.stringify(result.lines));
      const graph = analyzeCardBindings({ identity: 'actor', source: { name, layout: 'normal' }, faces: [{ sourceFaceIndex: 0, result }] });
      const actor = graph.references.find(ref => ref.path.endsWith('/effects/0/steps/0/who'));
      const later = graph.references.find(ref => ref.path.endsWith('/effects/1/steps/0/who'));
      assert.ok(actor?.binding === 'bound', JSON.stringify(actor));
      assert.ok(later?.binding === 'bound', JSON.stringify(later));
      assert.equal(actor.target, later.target);
      const event = graph.nodes.find(node => node.node === 'event' && node.path.endsWith('/effects/0/steps/0'));
      assert.ok(event?.node === 'event' && event.inputs.includes(actor.target), JSON.stringify(event));
    }
  });
});
