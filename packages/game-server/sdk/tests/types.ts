import type { CommandEnvelope, PlayerResponse, ExactQuantity, StreamPage, PlayerRequest, Publication, RulesResult, RejectionReason, CommandExecutionStatus, PublicManifest, Catalog, Decklist } from '../src/client.ts';

type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends (<T>() => T extends B ? 1 : 2) ? true : false;
type Assert<T extends true> = T;
type DamagePair = Assert<Equal<Extract<PlayerResponse, { response: 'damage' }>['assignments'][number], [string, ExactQuantity]>>;
type AllocationPair = Assert<Equal<Extract<PlayerResponse, { response: 'damage_allocation' }>['allocations'][number], [string, ExactQuantity]>>;
type AttackPair = Assert<Equal<Extract<PlayerResponse, { response: 'attackers' }>['attacks'][number], [string, number]>>;
type BlockPair = Assert<Equal<Extract<PlayerResponse, { response: 'blockers' }>['blocks'][number], [string, string]>>;
const damage: PlayerResponse = { response: 'damage', assignments: [['current-handle', '90071992547409931234567890']] };
// @ts-expect-error exact quantities are strings, never rounded JSON numbers.
const numericQuantity: PlayerResponse = { response: 'damage', assignments: [['h', 3]] };
// @ts-expect-error tuple arity comes from Rust Draft7 tuple schema.
const shortTuple: PlayerResponse = { response: 'damage', assignments: [['h']] };
// @ts-expect-error tuples cannot acquire an extra placement field.
const longTuple: PlayerResponse = { response: 'blockers', blocks: [['h', 'b', 'extra']] };
// @ts-expect-error old raw physical card IDs do not authorize player responses.
const physicalCast: PlayerResponse = { response: 'cast', card: { card: 0, incarnation: 0 } };
// @ts-expect-error transport revisions are canonical decimal strings.
const numericRevision: CommandEnvelope = { protocol_version: 1, command_id: '0'.repeat(32), expected_revision: 0, command: { kind: 'concede' } };
// @ts-expect-error public cursor is not a JavaScript number.
const numericCursor: StreamPage = { protocol_version: 1, entries: [], next: 0, has_more: false };
// @ts-expect-error unsupported protocol response cannot become unknown/any.
const rawReceipt: PlayerResponse = { response: 'raw_receipt', receipt: {} };

// A real consumer must account for every generated response variant. A Rust
// enum addition deliberately makes this compile check fail until reviewed.
export function exhaustive(response: PlayerResponse): string {
  switch (response.response) {
    case 'mulligan': return String(response.take);
    case 'bottom_opening': return response.objects.join(',');
    case 'damage_allocation': return String(response.allocations.length);
    case 'modes': return String(response.mode);
    case 'damage_effect': return response.effect;
    case 'order_triggers': return response.order.join(',');
    case 'entity_targets': return String(response.entities.length);
    case 'stack_targets': return response.entries.join(',');
    case 'activate_ability': return response.object;
    case 'player_targets': return response.players.join(',');
    case 'sacrifice': return response.object;
    case 'resolve_discard': return response.objects.join(',');
    case 'spend_mana': return JSON.stringify(response.spending);
    case 'commander_return': return String(response.to_command);
    case 'attackers': return String(response.attacks.length);
    case 'blockers': return String(response.blocks.length);
    case 'damage': return String(response.assignments.length);
    case 'discard': return response.objects.join(',');
    case 'play_land': return response.object;
    case 'activate_mana': return response.object;
    case 'cast': return response.card;
    case 'pass': return 'pass';
    case 'targets': return response.objects.join(',');
    case 'finish_mana': return String(response.undo_mana);
    case 'trample_damage': return `${response.player}:${response.amount}:${response.assignments.length}`;
    case 'search_lands': return response.objects.join(',');
    case 'sacrifice_cost': return response.object;
    case 'forced_sacrifice': return response.object;
    case 'optional_effect': return String(response.perform);
    case 'optional_recursion': return String(response.perform);
    case 'mana_color': return String(response.color);
    default: { const unhandled: never = response; return unhandled; }
  }
}
// Serialization-context Optional references remain required nullable fields.
type ReturnRequest = Extract<PlayerRequest, { request: 'commander_return' }>;
type RequiredNullableObject = Assert<Equal<ReturnRequest['object'], string | null>>;
void [damage, numericQuantity, shortTuple, longTuple, physicalCast, numericRevision, numericCursor, rawReceipt];
export type TupleProofs = [DamagePair, AllocationPair, AttackPair, BlockPair, RequiredNullableObject];

// Host pause/fault cannot be presented as a rules win or draw.
export function exhaustiveRulesResult(result: RulesResult): string {
  switch (result.outcome) {
    case 'win': return String(result.player);
    case 'draw': return 'draw';
    default: { const unhandled: never = result; return unhandled; }
  }
}
export function exhaustivePublication(publication: Publication): string {
  switch (publication.kind) {
    case 'view_replaced': return String(publication.observation.view.viewer);
    case 'lifecycle': return publication.status.status;
    case 'outcome': return exhaustiveRulesResult(publication.result);
    default: { const unhandled: never = publication; return unhandled; }
  }
}
// @ts-expect-error host resource exhaustion is not a Magic outcome.
const resourceIsNotRulesResult: RulesResult = { outcome: 'resource_paused' };
void resourceIsNotRulesResult;

export function exhaustiveRejectionReason(reason: RejectionReason): string {
  switch (reason) {
    case 'stale_revision':
    case 'invalid_response':
    case 'already_finished':
    case 'resource_paused':
    case 'cancelled': return reason;
    default: { const unhandled: never = reason; return unhandled; }
  }
}

export function exhaustiveExecution(status: CommandExecutionStatus): string {
  switch (status.status) {
    case 'pending':
    case 'consumed_by_rules':
    case 'discarded_by_departure':
    case 'cancelled': return status.status;
    case 'rejected': return exhaustiveRejectionReason(status.reason);
    default: { const unhandled: never = status; return unhandled; }
  }
}
// @ts-expect-error waiting is a host lifecycle, not command execution.
const waitingIsNotExecution: CommandExecutionStatus = { status: 'waiting' };
void waitingIsNotExecution;

// Catalog and deck data stay concrete generated Rust types throughout consumers.
export function publicCatalog(manifest: PublicManifest): [Catalog, Decklist] {
  return [manifest.catalog, manifest.decks[0]];
}
// @ts-expect-error public manifest never transports the actual match seed.
export type ForbiddenManifestSeed = PublicManifest['seed'];
// @ts-expect-error public manifest never transports a hidden profile digest.
export type ForbiddenManifestDigest = PublicManifest['profile_digest'];

// Newer hosted profiles retain exact tuples, private search handles and nested mana sources.
const trample: PlayerResponse = { response: 'trample_damage', assignments: [['blocker', '1']], player: 2, amount: '9007199254740993' };
const search: PlayerResponse = { response: 'search_lands', objects: ['private-resolution-handle'] };
const recursion: PlayerResponse = { response: 'optional_recursion', perform: true };
const color: PlayerResponse = { response: 'mana_color', color: 'C' };
// @ts-expect-error search responses use offered handles, never catalog IDs/rows.
const rawSearch: PlayerResponse = { response: 'search_lands', objects: [{ oracle_id: 'public-card' }] };
// @ts-expect-error color choices use the generated finite color enum.
const badColor: PlayerResponse = { response: 'mana_color', color: 'colorless' };
// @ts-expect-error trample quantities cannot be imprecise JSON numbers.
const numericTrample: PlayerResponse = { response: 'trample_damage', assignments: [], player: 1, amount: 3 };
void [trample, search, recursion, color, rawSearch, badColor, numericTrample];
