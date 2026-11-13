import type { SeatResume } from '@multiverse/game-server-sdk';
import type { PresentationArtifact } from '../../generated/presentation.d.ts';
import { cardView, inspectDecision } from './opening-fixture.ts';
import type { CardView, TableView } from './table-model.ts';

/** Group only the supplied permitted objects. Core owns visibility, current
 * characteristics, targets and combat; this adapter supplies no legal actions. */
export function liveTable(read: SeatResume, metadata: PresentationArtifact | null): TableView | null {
  if (read.state.status !== 'available') return null;
  if (metadata && metadata.catalog_sha256 !== read.catalog_sha256) throw new Error('Presentation catalog mismatch');
  const { view, decision } = read.state.observation;
  const display = (object: typeof view.objects[number]) => cardView(object, metadata);
  const stack = view.stack.map((entry, index): CardView => {
    const object = view.objects.find(object => object.handle === entry.spell);
    const source = entry.ability ?? entry.triggered;
    const printed = source && metadata?.cards.find(card => card.oracle_id === source.oracle_id);
    const face = printed?.faces.length === 1 ? printed.faces[0] : undefined;
    return {
      ...(object ? display(object) : {
        name: face?.name ?? null, printedType: face?.type_line ?? null, printedMana: face?.mana_cost ?? null,
        oracleText: face?.oracle_text ?? null,
        sourceIdentity: source ? `Oracle ${source.oracle_id} · Source handle ${source.source.current ?? source.source.announced}` : 'Source identity not supplied',
        currentFacts: [],
      }),
      inspectionId: entry.handle ?? source?.handle ?? `stack:${read.next}:${index}`, zoneLabel: 'Stack',
      currentFacts: [
        { label: 'Controller', value: `Seat ${entry.controller + 1}` },
        { label: 'Cast complete', value: entry.cast_complete ? 'Yes' : 'No' },
        ...(entry.mana_value !== undefined && entry.mana_value !== null ? [{ label: 'Mana value', value: entry.mana_value }] : []),
        ...(entry.x !== undefined && entry.x !== null ? [{ label: 'X', value: entry.x }] : []),
        ...entry.targets.map(target => ({ label: 'Supplied target', value: `Announced ${target.announced} · Current ${target.current ?? 'unavailable'}` })),
      ],
    };
  });
  const otherObjects = view.objects.filter(object => !['battlefield', 'command', 'stack'].includes(object.zone.zone)
    && !(object.zone.zone === 'hand' && object.owner === view.viewer));
  return {
    title: 'Commander table', live: true,
    sourceLabel: `Live permitted view · seat ${view.viewer + 1} · committed cursor ${read.next} · observation v${read.state.observation.observation_version}`,
    seats: view.players.map(player => ({
      label: `Seat ${player.player + 1}`, isViewer: player.player === view.viewer,
      life: player.life, handCount: String(player.hand_count), libraryCount: String(player.library_count),
      battlefield: view.objects.filter(object => object.zone.zone === 'battlefield' && object.controller === player.player).map(display),
      commandZone: view.objects.filter(object => object.zone.zone === 'command' && object.owner === player.player).map(display),
    })),
    hand: view.objects.filter(object => object.zone.zone === 'hand' && object.owner === view.viewer).map(display),
    commandZone: view.objects.filter(object => object.zone.zone === 'command' && object.owner === view.viewer).map(display),
    stack, otherObjects: otherObjects.map(display),
    combat: view.combat.map(attack => `${attack.attacker} → Seat ${attack.defender + 1} · ${attack.blocked ? 'Blocked' : 'Unblocked'}${attack.blockers.length ? ` · Blockers: ${attack.blockers.join(', ')}` : ''}`),
    turn: String(view.turn.number), phase: view.setup ? 'Opening hands' : view.turn.step.replaceAll('_', ' '),
    priority: view.priority === null ? 'None' : `Seat ${view.priority + 1}`, decision: inspectDecision(decision),
  };
}
