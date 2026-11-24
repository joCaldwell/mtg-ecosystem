import type { CommandEnvelope, Answer, ExactQuantity, StreamPage, Reason, Publication, RulesResult, RejectionReason, CommandExecutionStatus, PublicManifest, Catalog, Decklist } from '../src/client.ts';

type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends (<T>() => T extends B ? 1 : 2) ? true : false;
type Assert<T extends true> = T;
type NumberAmount = Assert<Equal<Extract<Answer,{answer:'number'}>['value'], ExactQuantity>>;
type ReturnObject = Assert<Equal<Extract<Reason,{reason:'commander_return'}>['object'], string | null>>;
export type WireProofs = [NumberAmount, ReturnObject];
export function exhaustive(answer: Answer): string {
  switch(answer.answer){
    case 'pick': case 'order': case 'number': case 'pay': case 'distribute': return answer.answer;
    default: { const unhandled:never=answer;return unhandled; }
  }
}
// @ts-expect-error candidate indices are numbers
const objectPick:Answer={answer:'pick', picks:[['object-handle']]};
// @ts-expect-error exact damage quantities are decimal strings
const rounded:Answer={answer:'distribute',amounts:[3]};
// @ts-expect-error revision counters are decimal strings
const revision:CommandEnvelope={protocol_version:2,command_id:'a'.repeat(32),expected_revision:0,command:{kind:'concede'}};
// @ts-expect-error stream cursors are decimal strings
const cursor:StreamPage={protocol_version:2,entries:[],next:0,has_more:false};
void [objectPick, rounded, revision, cursor];
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
