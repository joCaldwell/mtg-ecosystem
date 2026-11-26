import type { Observation, Decision, SeatResume } from '@multiverse/game-server-sdk';
import type { PresentationArtifact } from '../../../generated/presentation.d.ts';
import type { CardView, TableView } from '@multiverse/game-ui';

type VisibleObject = Observation['view']['objects'][number];

export function inspectDecision(request: Decision | null): TableView['decision'] {
  if (!request) return null;
  const reason = request.reason;
  if(reason.reason === 'mulligan') return {title:'Opening hand',description:'',details:[
    `Mulligans taken: ${reason.mulligans_taken}`,
    `Cards to bottom after a mulligan: ${reason.bottom_if_mulligan}`,
  ]};
  const title=reason.reason.replaceAll('_',' ');
  return {title:title[0]!.toUpperCase()+title.slice(1),description:'',details:[]};
}

export function cardView(object: VisibleObject, metadata: PresentationArtifact | null, showPrint = false): CardView {
  const printed = metadata?.cards.find(card => card.oracle_id === object.oracle_id);
  // Core names the active face of a double-faced or adventurer card (CR 712.8,
  // 715.3b); match producer faces only by that exact name. Never guess the
  // active face from Oracle text or array ordering.
  const faces = object.faces ?? null;
  const activeName = faces ? (faces.active_face === 'back' ? faces.back_name : faces.front_name) : null;
  const face = activeName === null
    ? (printed?.faces.length === 1 ? printed.faces[0] : undefined)
    : printed?.faces.find(candidate => candidate.name === activeName);
  const current = object.characteristics;
  const currentFacts: { label: string; value: string }[] = [
    { label: 'Owner', value: `Seat ${object.owner + 1}` },
    { label: 'Controller', value: `Seat ${object.controller + 1}` },
    ...(object.zone.zone === 'battlefield' ? [
      { label: 'Orientation', value: object.tapped ? 'Tapped' : 'Untapped' },
      { label: 'Marked damage', value: object.marked_damage },
    ] : []),
  ];
  if (current) {
    currentFacts.push({ label: 'Types', value: [current.kind, ...(current.additional_types ?? [])].join(' · ') });
    // CR 604.3, 613.1d: changeling-style "every creature type" is a flag, not a list.
    if (current.all_creature_types) currentFacts.push({ label: 'Creature types', value: 'All creature types' });
    if (current.subtypes.length) currentFacts.push({ label: 'Subtypes', value: current.subtypes.join(' · ') });
    if (current.power !== null) currentFacts.push({ label: 'Power', value: current.power });
    if (current.toughness !== null) currentFacts.push({ label: 'Toughness', value: current.toughness });
  }
  if (faces) {
    currentFacts.push({ label: 'Active face', value: `${faces.active_face === 'back' ? 'Back' : 'Front'} · ${activeName}` });
    if (faces.on_adventure) currentFacts.push({ label: 'On an adventure', value: 'Yes' });
  }
  if (object.attached_to) currentFacts.push({ label: 'Attached to', value: object.attached_to });
  if (object.attachments?.length) currentFacts.push({ label: 'Attachments', value: object.attachments.join(' · ') });
  if (object.plus_one_counters !== undefined) currentFacts.push({ label: '+1/+1 counters', value: object.plus_one_counters });
  return {
    inspectionId: object.handle, name: object.token?.name ?? face?.name ?? activeName,
    printedType: face?.type_line ?? null, printedMana: face?.mana_cost ?? null,
    oracleText: face?.oracle_text ?? null,
    ...(showPrint && !object.token && face?.image_url ? { printImage: face.image_url } : {}),
    ...(object.zone.zone === 'battlefield' ? { tapped: object.tapped } : {}),
    sourceIdentity: `Oracle ${object.oracle_id}${printed ? ` · Printing ${printed.printing_id} · Source SHA-256 ${printed.source_sha256}` : ' · Presentation metadata unavailable'}`,
    zoneLabel: `Seat ${object.owner + 1} · ${object.zone.zone === 'command' ? 'Command zone' : object.zone.zone[0]!.toUpperCase() + object.zone.zone.slice(1)}`,
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
  const display = (object: VisibleObject) => cardView(object, metadata, true);
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
