import type { DecisionContext, PlayerRequest, PlayerResponse } from '@multiverse/game-server-sdk';
import type { ChoiceRow, DecisionSurface, LocalDraft, ObjectLabel, Option, QuantityField } from './controls.ts';

// Local control identities are encoded so an arbitrary permitted handle cannot
// collide with the explicit unused choice. These are never gameplay wire IDs.
const unused = 'unused';
const identity = (value: string | number) => JSON.stringify([value]);
const read = (draft: LocalDraft, id: string) => draft.values?.find(([key]) => key === id)?.[1] ?? '';
const object = (handle: string, labels: readonly ObjectLabel[]): Option => ({ id: identity(handle), label: labels.find(label => label.handle === handle)?.name ?? 'Unidentified object', detail: handle });
const player = (seat: number): Option => ({ id: identity(seat), label: `Seat ${seat + 1}`, detail: '' });
const unavailable = (): DecisionSurface => ({ status: 'context-missing', title: 'Decision context unavailable', reason: 'The permitted source or instruction needed to explain this choice is unavailable. No draft can be submitted.' });
const supported = (control: Extract<DecisionSurface, { status: 'supported' }>['control']): DecisionSurface => ({ status: 'supported', control });
const maxDigits = 80; // Local input-size bound, never a Magic quantity maximum.
const colors = ['W', 'U', 'B', 'R', 'G', 'C'] as const;
const colorNames = { W: 'White', U: 'Blue', B: 'Black', R: 'Red', G: 'Green', C: 'Colorless' };
const choice = (title: string, description: string, options: readonly Option[]): DecisionSurface => supported({ kind: 'choice', title, description, options });
const mapping = (title: string, description: string, rows: readonly ChoiceRow[]): DecisionSurface => supported({ kind: 'mapping', title, description, rows });
const quantities = (title: string, description: string, fields: readonly QuantityField[], total?: string): DecisionSurface => supported({ kind: 'quantities', title, description, fields, maxDigits, ...(total === undefined ? {} : { total }) });

/** Only producer-owned context is explanatory authority. Complete source text is
 * displayed verbatim; this module never parses it or interprets TargetSpec/IR. */
export function structuredDecisionSurface(request: PlayerRequest, labels: readonly ObjectLabel[], context?: DecisionContext | null): DecisionSurface | null {
  const source = context?.source?.text;
  const instruction = context?.instruction?.text;
  const sourceIdentity = context?.source ? context.source.object ? `${object(context.source.object, labels).label} (${context.source.object})` : `Source identity: ${context.source.oracle_id}` : '';
  const explanation = [sourceIdentity, source, instruction].filter(Boolean).join('\n\n');
  switch (request.request) {
    case 'announce_x': return source && context?.source?.kind === 'spell'
      ? supported({ kind: 'quantity', title: 'Choose X', description: `${explanation}\nEnter X explicitly. The table validates its meaning and value.`, maxDigits }) : unavailable();
    case 'mana_color': return source && context?.source?.kind === 'mana_ability'
      ? choice('Choose mana color', explanation, request.colors.map(color => ({ id: color, label: colorNames[color], detail: '' }))) : unavailable();
    case 'optional_effect': case 'optional_recursion': return source && instruction
      ? choice('Choose whether to perform the effect', explanation, [{ id: 'perform', label: 'Perform this effect', detail: '' }, { id: 'decline', label: 'Decline this effect', detail: '' }]) : unavailable();
    case 'modes': {
      if (!source || !request.candidates.every(mode => context?.modes?.filter(label => label.mode === mode && label.text).length === 1)) return unavailable();
      return choice('Choose a mode', explanation, request.candidates.map(mode => ({ id: String(mode), label: context!.modes!.find(label => label.mode === mode)!.text, detail: '' })));
    }
    case 'order_triggers': {
      if (!request.candidates.every(candidate => candidate.label)) return unavailable();
      return supported({ kind: 'selection', title: 'Order your triggered abilities', description: 'Choose every supplied trigger and review the order, first to last. The table validates the final ordering.', options: request.candidates.map(candidate => ({ id: candidate.handle, label: candidate.label!, detail: candidate.handle })), minimum: request.candidates.length, maximum: request.candidates.length, ordered: true });
    }
    case 'damage_effect': {
      if (!request.candidates.every(candidate => candidate.label)) return unavailable();
      const recipient = request.recipient.entity === 'player' ? player(request.recipient.player).label : `${object(request.recipient.object, labels).label} (${request.recipient.object})`;
      return choice('Choose a damage effect', `Recipient: ${recipient}. Choose from the supplied effects. The table validates the selected effect and damage occurrence.`, request.candidates.map(candidate => ({ id: JSON.stringify([candidate.effect, candidate.damage]), label: candidate.label!, detail: `${candidate.effect_source.announced} · Amount ${candidate.amount_before} → ${candidate.amount_after}${candidate.budget_remaining === null ? '' : ` · Remaining ${candidate.budget_remaining}`}` })));
    }
    case 'damage_allocation': {
      if (!source || !instruction || context?.source?.kind !== 'effect') return unavailable();
      return quantities('Allocate the supplied damage prevention', `${explanation}\nSupplied required allocation: ${request.required}. Enter every amount, including zero. The table validates the complete allocation.`, request.candidates.map(candidate => ({ id: candidate.damage, label: object(candidate.source.current ?? candidate.source.announced, labels).label, detail: `${candidate.damage} · Source ${candidate.source.announced} · Supplied amount ${candidate.amount}`, maximum: candidate.amount })), request.required);
    }
    case 'commander_return': {
      const label = request.object === null ? `Commander ${request.designation}` : `${object(request.object, labels).label} (${request.object})`;
      const previous = (request.previous_choices ?? []).map(value => `${value.designation}: ${value.to_command ? 'command zone' : 'destination zone'}`).join('; ');
      return choice('Choose the commander destination', `${label}. Destination: ${request.destination.zone}.${previous ? ` Previous choices: ${previous}.` : ''}`, [{ id: 'command', label: 'Move to the command zone', detail: '' }, { id: 'destination', label: `Keep the destination: ${request.destination.zone}`, detail: '' }]);
    }
    case 'spend_mana': {
      if (!context?.source) return unavailable();
      const fields = colors.filter(color => request.available[color] !== undefined).map(color => ({ id: color, label: colorNames[color], detail: `Available: ${request.available[color]}`, maximum: request.available[color]! }));
      return quantities('Choose mana to spend', `${explanation}\nSupplied cost: generic ${request.cost.generic}; colored ${request.cost.colored.join(', ') || 'none'}. Enter every amount, including zero. The table validates the complete payment.`, fields);
    }
    case 'attackers': return mapping('Declare attackers', 'For each supplied attacker, choose a supplied defending seat or explicitly leave it out of combat. The table validates the complete declaration.', request.candidates.map(handle => ({ ...object(handle, labels), id: handle, options: [{ id: unused, label: 'Do not attack', detail: '' }, ...request.defenders.map(player)] })));
    case 'blockers': {
      // An omitted legal map is unavailable, never authority to offer every
      // attacker. Supplied menace hints are shown, not recomputed from text.
      if (request.candidates.some(handle => (request.legal_attackers ?? []).filter(row => row.blocker === handle).length !== 1)) return unavailable();
      const menace = (request.menace_attackers ?? []).map(handle => `${object(handle, labels).label} (${handle})`).join(', ');
      const rows = request.candidates.map(handle => ({ ...object(handle, labels), id: handle, options: [{ id: unused, label: 'Do not block', detail: '' }, ...request.legal_attackers!.find(row => row.blocker === handle)!.attackers.filter(attacker => request.attackers.includes(attacker)).map(attacker => object(attacker, labels))] }));
      return mapping('Declare blockers', `Choose only from each blocker's supplied attackers. The table validates all joint restrictions.${menace ? ` Supplied menace attackers: ${menace}.` : ''}`, rows);
    }
    case 'damage': {
      const fields: QuantityField[] = request.recipients.map(handle => ({ ...object(handle, labels), detail: `${handle}${request.lethal_requirements?.some(([recipient]) => recipient === handle) ? ` · Supplied lethal requirement: ${request.lethal_requirements.find(([recipient]) => recipient === handle)![1]}` : ''}` }));
      if (request.player_recipient !== undefined && request.player_recipient !== null) fields.push({ ...player(request.player_recipient), id: 'player', detail: 'Supplied player recipient' });
      return quantities('Assign combat damage', `Source: ${object(request.source, labels).label} (${request.source}). Supplied power: ${request.power}. Enter every amount, including zero. The table validates assignment order and lethal requirements.`, fields, request.power);
    }
    case 'targets': case 'entity_targets': case 'player_targets': case 'stack_targets': {
      if (!source) return unavailable();
      const options: Option[] = request.request === 'player_targets' ? request.candidates.map(player)
        : request.request === 'entity_targets' ? request.candidates.map(candidate => candidate.entity === 'player' ? player(candidate.player) : object(candidate.object, labels))
        : request.candidates.map(handle => ({ ...object(handle, labels), label: request.request === 'stack_targets' ? `Stack entry ${handle}` : object(handle, labels).label }));
      return mapping('Choose targets in the supplied order', `${explanation}\nChoose one candidate for each supplied occurrence. The table validates the combined targets.`, request.occurrences.map((_, index) => ({ id: String(index), label: `Target ${index + 1}`, detail: '', options })));
    }
    default: return null;
  }
}

/** Called only after membership, explicit completion and exact syntax checks. */
export function completeStructuredDecision(request: PlayerRequest, draft: LocalDraft): PlayerResponse | undefined {
  switch (request.request) {
    case 'announce_x': return { response: 'announce_x', value: draft.quantity };
    case 'mana_color': return { response: 'mana_color', color: request.colors.find(color => color === draft.selected[0])! };
    case 'optional_effect': case 'optional_recursion': return { response: request.request, perform: draft.selected[0] === 'perform' };
    case 'modes': return { response: 'modes', mode: request.candidates.find(mode => String(mode) === draft.selected[0])! };
    case 'order_triggers': return { response: 'order_triggers', order: [...draft.selected] };
    case 'damage_effect': {
      const candidate = request.candidates.find(value => JSON.stringify([value.effect, value.damage]) === draft.selected[0])!;
      return { response: 'damage_effect', effect: candidate.effect, damage: candidate.damage };
    }
    case 'damage_allocation': return { response: 'damage_allocation', allocations: request.candidates.map(candidate => [candidate.damage, read(draft, candidate.damage)]) };
    case 'commander_return': return { response: 'commander_return', to_command: draft.selected[0] === 'command' };
    // Native selected_payment_pool requires sparse, strictly positive entries.
    // Explicit zero inputs remain visible draft intent but add no wire entry.
    case 'spend_mana': return { response: 'spend_mana', spending: Object.fromEntries(colors.filter(color => request.available[color] !== undefined && read(draft, color) !== '0').map(color => [color, read(draft, color)])) };
    case 'attackers': return { response: 'attackers', attacks: request.candidates.filter(handle => read(draft, handle) !== unused).map(handle => [handle, request.defenders.find(seat => identity(seat) === read(draft, handle))!]) };
    case 'blockers': return { response: 'blockers', blocks: request.candidates.filter(handle => read(draft, handle) !== unused).map(handle => [handle, request.attackers.find(attacker => identity(attacker) === read(draft, handle))!]) };
    case 'damage': {
      const assignments: [string, string][] = request.recipients.map(handle => [handle, read(draft, identity(handle))]);
      return request.player_recipient === undefined || request.player_recipient === null ? { response: 'damage', assignments }
        : { response: 'trample_damage', assignments, player: request.player_recipient, amount: read(draft, 'player') };
    }
    case 'targets': return { response: 'targets', objects: request.occurrences.map((_, index) => request.candidates.find(handle => identity(handle) === read(draft, String(index)))!) };
    case 'stack_targets': return { response: 'stack_targets', entries: request.occurrences.map((_, index) => request.candidates.find(handle => identity(handle) === read(draft, String(index)))!) };
    case 'player_targets': return { response: 'player_targets', players: request.occurrences.map((_, index) => request.candidates.find(seat => identity(seat) === read(draft, String(index)))!) };
    case 'entity_targets': return { response: 'entity_targets', entities: request.occurrences.map((_, index) => request.candidates.find(candidate => identity(candidate.entity === 'player' ? candidate.player : candidate.object) === read(draft, String(index)))!) };
    default: return undefined;
  }
}
