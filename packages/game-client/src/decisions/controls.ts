import type { PlayerRequest, PlayerResponse } from '@multiverse/game-server-sdk';
import { parseQuantity } from './quantity.ts';

export type DecisionRequest = PlayerRequest;
export interface DecisionCompletion { readonly binding: DecisionBinding; readonly response: PlayerResponse }

/** Local identity, not authorization or a wire DTO. The app must retain the
 * original session CommandContext with this binding; never attach a fresh one
 * to an old completion. Incarnation is the controller's generation. */
export interface DecisionBinding {
  readonly origin: string;
  readonly match: string;
  readonly seat: PlayerRequest['player'];
  readonly incarnation: number;
  readonly revision: string;
  readonly token: PlayerRequest['token'];
}
export function bindingKey(binding: DecisionBinding): string {
  return JSON.stringify([binding.origin, binding.match, binding.seat, binding.incarnation, binding.revision, binding.token]);
}
export interface Option { readonly id: string; readonly label: string; readonly detail: string }
interface Base { readonly title: string; readonly description: string }
/** Presentation models only. No alternative gameplay request/response types. */
export type LocalControl = Base & (
  | { readonly kind: 'choice'; readonly options: readonly Option[] }
  | { readonly kind: 'selection'; readonly options: readonly Option[]; readonly minimum: number; readonly maximum: number; readonly ordered: boolean }
  | { readonly kind: 'quantity'; readonly maxDigits: number }
);
export type DecisionSurface = { readonly status: 'supported'; readonly control: LocalControl }
  | { readonly status: 'context-missing' | 'unsupported'; readonly title: string; readonly reason: string };
export interface LocalDraft { readonly selected: readonly string[]; readonly quantity: string }
export const emptyDraft: LocalDraft = Object.freeze({ selected: Object.freeze([]), quantity: '' });
/** Labels must come only from the same permitted observation and producer
 * presentation mapping. Missing metadata preserves the exact handle. */
export interface ObjectLabel { readonly handle: string; readonly name: string | null }
function options(handles: readonly string[], labels: readonly ObjectLabel[]): readonly Option[] {
  return handles.map(handle => ({ id: handle, label: labels.find(label => label.handle === handle)?.name ?? 'Unidentified object', detail: handle }));
}
const choice = (title: string, description: string, values: readonly (readonly [string, string])[]): DecisionSurface => ({
  status: 'supported', control: { kind: 'choice', title, description, options: values.map(([id, label]) => ({ id, label, detail: '' })) },
});

/** Exhaustive over generated Rust requests. Supplied membership/count bookkeeping
 * never establishes joint legality. Ambiguous semantic context stays blocked. */
export function decisionSurface(request: PlayerRequest, labels: readonly ObjectLabel[]): DecisionSurface {
  const selection = (title: string, handles: readonly string[], minimum: number, maximum: number, ordered = false): DecisionSurface => ({
    status: 'supported', control: { kind: 'selection', title, description: 'Choose from the supplied handles. Core must validate the complete response.', options: options(handles, labels), minimum, maximum, ordered },
  });
  switch (request.request) {
    case 'mulligan': return choice('Opening hand', `Mulligans taken: ${request.mulligans_taken}. Cards to bottom if taking this mulligan: ${request.bottom_if_mulligan}.`,
      request.can_mulligan ? [['keep', 'Keep hand'], ['mulligan', 'Take mulligan']] : [['keep', 'Keep hand']]);
    case 'bottom_opening': return selection('Order cards for the bottom of your library', request.candidates, request.count, request.count, true);
    case 'discard': case 'resolve_discard': return selection('Choose cards to discard', request.candidates, request.count, request.count);
    case 'search_lands': return selection('Choose supplied search results', request.candidates.map(candidate => candidate.handle), 0, request.maximum);
    case 'legend_rule': return selection('Choose the object to keep', request.candidates, 1, 1);
    case 'sacrifice_cost': case 'forced_sacrifice': case 'sacrifice': return selection('Choose an object to sacrifice', request.candidates, 1, 1);
    case 'announce_x': case 'mana_color': case 'optional_effect': case 'optional_recursion':
    case 'modes': case 'order_triggers': case 'damage_effect': case 'damage_allocation':
    case 'commander_return': return { status: 'context-missing', title: 'Decision context unavailable', reason: `${request.request}: source, effect or source-label mapping is not sufficient for a reviewed human choice. No draft is available.` };
    case 'priority': case 'mana': return { status: 'context-missing', title: 'Action labels unavailable', reason: `${request.request}: requires the bound core action query and human-readable source mapping. An ability index is not a human label.` };
    case 'spend_mana': case 'targets': case 'entity_targets': case 'player_targets': case 'stack_targets':
    case 'attackers': case 'blockers': case 'damage': return { status: 'unsupported', title: 'Control not implemented', reason: `${request.request}: needs reviewed joint constraints and permitted context integration. No costs, targets or lethal requirements are derived here.` };
    default: { const exhaustive: never = request; return exhaustive; }
  }
}

/** New candidate incarnations never inherit selection by label. */
export function reconcileDraft(draft: LocalDraft, control: LocalControl): LocalDraft {
  const selected = control.kind === 'quantity' ? [] : draft.selected.filter(id => control.options.some(option => option.id === id));
  return { selected: [...new Set(selected)], quantity: draft.quantity };
}
export function draftError(control: LocalControl, draft: LocalDraft): string | null {
  if (control.kind === 'quantity') return parseQuantity(draft.quantity, control.maxDigits) === null ? `Enter a canonical nonnegative integer, at most ${control.maxDigits} digits.` : null;
  if (new Set(control.options.map(option => option.id)).size !== control.options.length) return 'Candidate identity is ambiguous.';
  if (new Set(draft.selected).size !== draft.selected.length || draft.selected.some(id => !control.options.some(option => option.id === id))) return 'Selection is no longer in the supplied candidates.';
  const minimum = control.kind === 'choice' ? 1 : control.minimum;
  const maximum = control.kind === 'choice' ? 1 : control.maximum;
  if (!Number.isSafeInteger(minimum) || !Number.isSafeInteger(maximum) || minimum < 0 || maximum < minimum) return 'Supplied count is unavailable.';
  return draft.selected.length < minimum || draft.selected.length > maximum ? `Choose ${minimum === maximum ? minimum : `${minimum}–${maximum}`} item(s).` : null;
}
export function toggleDraft(control: LocalControl, draft: LocalDraft, id: string): LocalDraft {
  if (control.kind === 'quantity' || !control.options.some(option => option.id === id)) return draft;
  if (control.kind === 'choice') return { ...draft, selected: [id] };
  if (draft.selected.includes(id)) return { ...draft, selected: draft.selected.filter(value => value !== id) };
  return draft.selected.length < control.maximum ? { ...draft, selected: [...draft.selected, id] } : draft;
}
export function reorderDraft(draft: LocalDraft, id: string, offset: -1 | 1): LocalDraft {
  const from = draft.selected.indexOf(id), to = from + offset;
  if (from < 0 || to < 0 || to >= draft.selected.length) return draft;
  const selected = [...draft.selected];
  selected.splice(from, 1); selected.splice(to, 0, id);
  return { ...draft, selected };
}

/** Returns only generated responses for explicitly implemented request branches.
 * No default values, guesses, implicit pass, or side effects. */
export function completeDecision(request: PlayerRequest, labels: readonly ObjectLabel[], draft: LocalDraft): PlayerResponse | null {
  const surface = decisionSurface(request, labels);
  if (surface.status !== 'supported' || draftError(surface.control, draft)) return null;
  const objects = [...draft.selected];
  const object = objects[0];
  switch (request.request) {
    case 'mulligan': return { response: 'mulligan', take: objects[0] === 'mulligan' };
    case 'bottom_opening': return { response: 'bottom_opening', objects };
    case 'discard': return { response: 'discard', objects };
    case 'resolve_discard': return { response: 'resolve_discard', objects };
    case 'search_lands': return { response: 'search_lands', objects };
    case 'legend_rule': return object === undefined ? null : { response: 'legend_rule', keep: object };
    case 'sacrifice_cost': return object === undefined ? null : { response: 'sacrifice_cost', object };
    case 'forced_sacrifice': return object === undefined ? null : { response: 'forced_sacrifice', object };
    case 'sacrifice': return object === undefined ? null : { response: 'sacrifice', object };
    case 'announce_x': case 'mana_color': case 'optional_effect': case 'optional_recursion':
    case 'modes': case 'order_triggers': case 'damage_effect': case 'damage_allocation':
    case 'commander_return': case 'priority': case 'mana': case 'spend_mana': case 'targets':
    case 'entity_targets': case 'player_targets': case 'stack_targets': case 'attackers': case 'blockers': case 'damage': return null;
    default: { const exhaustive: never = request; return exhaustive; }
  }
}
