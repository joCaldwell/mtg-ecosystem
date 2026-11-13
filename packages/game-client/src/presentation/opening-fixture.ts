import type { Observation, PlayerRequest, SeatResume } from '@multiverse/game-server-sdk';
import type { PresentationArtifact } from '../../generated/presentation.d.ts';
import type { CardView, TableView } from './table-model.ts';

type VisibleObject = Observation['view']['objects'][number];

export function inspectDecision(request: PlayerRequest | null): TableView['decision'] {
  if (!request) return null;
  switch (request.request) {
    case 'mulligan': return {
      title: 'Opening hand',
      description: 'Inspect your hand before choosing whether to keep it.',
      details: [`Mulligans taken: ${request.mulligans_taken}`, `Mulligan available: ${request.can_mulligan ? 'yes' : 'no'}`, `Cards to bottom if taking this mulligan: ${request.bottom_if_mulligan}`],
    };
    case 'legend_rule': case 'announce_x': case 'sacrifice_cost': case 'forced_sacrifice':
    case 'mana_color': case 'search_lands': case 'optional_effect': case 'optional_recursion':
    case 'bottom_opening': case 'damage_allocation': case 'modes': case 'damage_effect':
    case 'order_triggers': case 'entity_targets': case 'stack_targets': case 'player_targets':
    case 'sacrifice': case 'resolve_discard': case 'spend_mana': case 'commander_return':
    case 'attackers': case 'blockers': case 'damage': case 'discard': case 'priority':
    case 'targets': case 'mana': return {
      title: 'Unsupported decision',
      description: 'This build has no reviewed inspector for this decision. No choice can be submitted.',
      details: [`Request: ${request.request}`],
    };
    default: {
      const exhaustive: never = request;
      return exhaustive;
    }
  }
}

export function cardView(object: VisibleObject, metadata: PresentationArtifact | null): CardView {
  const printed = metadata?.cards.find(card => card.oracle_id === object.oracle_id);
  // Multi-face active identity is a separate producer/core mapping gate. Do not
  // guess the active face from Oracle text or array ordering.
  const face = printed?.faces.length === 1 ? printed.faces[0] : undefined;
  const current = object.characteristics;
  const currentFacts: { label: string; value: string }[] = [
    { label: 'Owner', value: `Seat ${object.owner + 1}` },
    { label: 'Controller', value: `Seat ${object.controller + 1}` },
    { label: 'Orientation', value: object.tapped ? 'Tapped' : 'Untapped' },
    { label: 'Marked damage', value: object.marked_damage },
  ];
  if (current) {
    currentFacts.push({ label: 'Types', value: [current.kind, ...(current.additional_types ?? [])].join(' · ') });
    if (current.subtypes.length) currentFacts.push({ label: 'Subtypes', value: current.subtypes.join(' · ') });
    if (current.power !== null) currentFacts.push({ label: 'Power', value: current.power });
    if (current.toughness !== null) currentFacts.push({ label: 'Toughness', value: current.toughness });
  }
  if (object.attached_to) currentFacts.push({ label: 'Attached to', value: object.attached_to });
  if (object.attachments?.length) currentFacts.push({ label: 'Attachments', value: object.attachments.join(' · ') });
  if (object.plus_one_counters !== undefined) currentFacts.push({ label: '+1/+1 counters', value: object.plus_one_counters });
  return {
    inspectionId: object.handle, name: object.token?.name ?? face?.name ?? null,
    printedType: face?.type_line ?? null, printedMana: face?.mana_cost ?? null,
    oracleText: face?.oracle_text ?? null,
    sourceIdentity: `Oracle ${object.oracle_id}${printed ? ` · Printing ${printed.printing_id} · Source SHA-256 ${printed.source_sha256}` : ' · Presentation metadata unavailable'}`,
    zoneLabel: `Seat ${object.owner + 1} · ${object.zone.zone === 'command' ? 'Command zone' : object.zone.zone}`,
    currentFacts,
  };
}

/** Small reviewed opening-view adapter, not a general game projection. Input is
 * trusted only after pinned fixture verification; live SDK validation is separate. */
export function openingFixtureTable(resume: SeatResume, metadata: PresentationArtifact): TableView {
  if (resume.catalog_sha256 !== metadata.catalog_sha256) throw new Error('Presentation catalog mismatch');
  if (resume.state.status !== 'available') throw new Error('Fixture observation unavailable');
  const { view, decision } = resume.state.observation;
  if (view.viewer !== resume.viewer) throw new Error('Fixture viewer mismatch');
  if (view.stack.length || view.combat.length || view.objects.some(object => !['hand', 'command', 'battlefield'].includes(object.zone.zone))) throw new Error('Unsupported opening fixture shape');
  const display = (object: VisibleObject) => cardView(object, metadata);
  return {
    title: 'Commander table',
    sourceLabel: `Foundations opening fixture · seat ${view.viewer + 1} · committed cursor ${resume.next} · observation v${resume.state.observation.observation_version}`,
    seats: view.players.map(player => ({
      label: `Seat ${player.player + 1}`, isViewer: player.player === view.viewer,
      life: player.life, handCount: String(player.hand_count), libraryCount: String(player.library_count),
      battlefield: view.objects.filter(object => object.zone.zone === 'battlefield' && object.controller === player.player).map(display),
      commandZone: view.objects.filter(object => object.zone.zone === 'command' && object.owner === player.player).map(display),
    })),
    hand: view.objects.filter(object => object.zone.zone === 'hand' && object.owner === view.viewer).map(display),
    commandZone: view.objects.filter(object => object.zone.zone === 'command' && object.owner === view.viewer).map(display),
    stack: [], turn: String(view.turn.number),
    phase: view.setup ? 'Opening hands' : view.turn.step.replaceAll('_', ' '),
    priority: view.priority === null ? 'None' : `Seat ${view.priority + 1}`,
    decision: inspectDecision(decision),
  };
}
