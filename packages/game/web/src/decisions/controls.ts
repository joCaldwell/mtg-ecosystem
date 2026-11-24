import type { DecisionContext, Decision, Answer } from '@multiverse/game-server-sdk';
import { parseQuantity } from './quantity.ts';
export type { DecisionContext } from '@multiverse/game-server-sdk';

export type DecisionRequest = Decision;
export interface DecisionCompletion {
  readonly binding: DecisionBinding;
  readonly response: Answer;
}

/** Local identity, not authorization or a wire DTO. The app must retain the
 * original session CommandContext with this binding; never attach a fresh one
 * to an old completion. Incarnation is the controller's generation. */
export interface DecisionBinding {
  readonly origin: string;
  readonly match: string;
  readonly seat: Decision['player'];
  readonly incarnation: number;
  readonly revision: string;
  readonly token: Decision['token'];
}
export function bindingKey(binding: DecisionBinding): string {
  return JSON.stringify([
    binding.origin,
    binding.match,
    binding.seat,
    binding.incarnation,
    binding.revision,
    binding.token,
  ]);
}
export interface Option {
  readonly id: string;
  readonly label: string;
  readonly detail: string;
}
export interface ChoiceRow extends Option {
  readonly options: readonly Option[];
}
export interface QuantityField extends Option {
  readonly maximum?: string;
}
interface Base {
  readonly shape: Decision['shape'];
  readonly title: string;
  readonly description: string;
}
/** Presentation models only. No alternative gameplay request/response types. */
export type LocalControl = Base &
  (
    | { readonly kind: 'choice'; readonly options: readonly Option[] }
    | {
        readonly kind: 'selection';
        readonly options: readonly Option[];
        readonly minimum: number;
        readonly maximum: number;
        readonly ordered: boolean;
      }
    | { readonly kind: 'quantity'; readonly maxDigits: number }
    | {
        readonly kind: 'counts';
        readonly slots: readonly {
          readonly label: string;
          readonly options: readonly Option[];
          readonly minimum: number;
          readonly maximum: number;
          readonly repeat: boolean;
        }[];
      }
    | { readonly kind: 'mapping'; readonly rows: readonly ChoiceRow[] }
    | {
        readonly kind: 'quantities';
        readonly fields: readonly QuantityField[];
        readonly maxDigits: number;
        readonly total?: string;
      }
  );
export type DecisionSurface =
  | { readonly status: 'supported'; readonly control: LocalControl }
  | { readonly status: 'context-missing' | 'unsupported'; readonly title: string; readonly reason: string };
export interface LocalDraft {
  readonly selected: readonly string[];
  readonly quantity: string;
  readonly values?: readonly (readonly [string, string])[];
}
export const emptyDraft: LocalDraft = Object.freeze({ selected: Object.freeze([]), quantity: '' });
/** Labels must come only from the same permitted observation and producer
 * presentation mapping. Missing metadata preserves the exact handle. */
export interface ObjectLabel {
  readonly handle: string;
  readonly name: string | null;
}
export function decisionSurface(
  request: Decision,
  labels: readonly ObjectLabel[],
  context?: DecisionContext | null,
): DecisionSurface {
  const { shape } = request;
  const base = {
    shape,
    title: request.reason.reason.replaceAll('_', ' '),
    description: context?.instruction?.text ?? context?.source?.text ?? '',
  };
  const supported = (control: LocalControl): DecisionSurface => ({ status: 'supported', control });
  const option = (index: number): Option => {
    const c = request.candidates[index];
    const label = context?.candidates?.[index];
    const identity =
      c?.candidate === 'object'
        ? c.object
        : c?.candidate === 'action' && 'object' in c.action
          ? c.action.object
          : c?.candidate === 'action' && 'card' in c.action
            ? c.action.card
            : null;
    const name = identity && labels.find((l) => l.handle === identity)?.name;
    if (c?.candidate === 'object')
      return {
        id: String(index),
        label: name || (label?.text !== identity && label?.text) || 'Unidentified object',
        detail: c.object,
      };
    const fallback =
      c?.candidate === 'action'
        ? c.action.action
        : c?.candidate === 'player'
          ? `Seat ${c.player + 1}`
          : c?.candidate === 'flag'
            ? {
                keep: 'Keep hand',
                mulligan: 'Take mulligan',
                accept: 'Accept',
                decline: 'Decline',
                command_zone: 'Command zone',
                destination: 'Destination',
              }[c.flag]
            : c?.candidate === 'color'
              ? c.color
              : (c?.candidate ?? 'Option');
    return {
      id: String(index),
      label: [label?.text || fallback.replaceAll('_', ' '), name].filter(Boolean).join(' · '),
      detail: label?.source?.text ?? identity ?? '',
    };
  };
  switch (shape.shape) {
    case 'pick': {
      const slot = shape.slots[0];
      if (shape.slots.some((s) => s.repeat || ((shape.slots.length > 1 || s.subject != null) && s.max > 1))) {
        return supported({
          ...base,
          kind: 'counts',
          slots: shape.slots.map((s, i) => ({
            label: s.subject == null ? `Choice ${i + 1}` : option(s.subject).label,
            minimum: s.min,
            maximum: s.max,
            repeat: s.repeat,
            options: s.allowed.map(option),
          })),
        });
      }
      if (shape.slots.length === 1 && slot && slot.subject == null) {
        const options = slot.allowed.map(option);
        return slot.min === 1 && slot.max === 1
          ? supported({ ...base, kind: 'choice', options })
          : supported({ ...base, kind: 'selection', minimum: slot.min, maximum: slot.max, ordered: false, options });
      }
      return supported({
        ...base,
        kind: 'mapping',
        rows: shape.slots.map((slot, i) => {
          const subject = slot.subject == null ? { label: `Choice ${i + 1}`, detail: '' } : option(slot.subject);
          return {
            ...subject,
            id: String(i),
            options: [
              ...(slot.min === 0 ? [{ id: 'unused', label: 'None', detail: '' }] : []),
              ...slot.allowed.map(option),
            ],
          };
        }),
      });
    }
    case 'order':
      return supported({
        ...base,
        kind: 'selection',
        ordered: true,
        options: shape.items.map(option),
        minimum: shape.min,
        maximum: shape.max,
      });
    case 'number':
      return supported({ ...base, kind: 'quantity', maxDigits: 65536 });
    case 'distribute':
      return supported({
        ...base,
        kind: 'quantities',
        maxDigits: 65536,
        total: shape.total,
        fields: shape.buckets.map((b, i) => ({ ...option(b.candidate), id: String(i), maximum: b.max })),
      });
    case 'pay':
      if (shape.step.step === 'produce')
        return supported({
          ...base,
          kind: 'choice',
          options: [
            ...shape.step.sources.map(option),
            { id: 'finish-undo', label: 'Finish payment and undo mana if it fails', detail: '' },
            { id: 'finish', label: 'Finish payment and keep mana if it fails', detail: '' },
          ],
        });
      return supported({
        ...base,
        kind: 'quantities',
        maxDigits: 65536,
        fields: Object.entries(shape.pool.unrestricted).map(([color, maximum]) => ({
          id: color,
          label: color,
          detail: `Available ${maximum}`,
          maximum,
        })),
        total: String(BigInt(shape.cost.generic) + BigInt(shape.cost.colored.length)),
      });
  }
}
/** New candidate incarnations never inherit selection by label. */
export function reconcileDraft(draft: LocalDraft, control: LocalControl): LocalDraft {
  if (control.kind === 'counts') return emptyDraft;
  if (control.kind === 'mapping' || control.kind === 'quantities') {
    const fields = control.kind === 'mapping' ? control.rows : control.fields;
    return {
      ...emptyDraft,
      values: (draft.values ?? []).filter(([id, value]) =>
        fields.some(
          (field) =>
            field.id === id &&
            (control.kind !== 'mapping' ||
              control.rows.find((row) => row.id === id)?.options.some((option) => option.id === value)),
        ),
      ),
    };
  }
  const selected =
    control.kind === 'quantity'
      ? []
      : draft.selected.filter((id) => control.options.some((option) => option.id === id));
  return { selected: [...new Set(selected)], quantity: draft.quantity };
}
function fieldError(control: LocalControl, draft: LocalDraft): string | null {
  if (control.kind === 'counts') {
    const fields = new Set<string>();
    for (const [i, slot] of control.slots.entries()) {
      let total = 0n;
      for (const option of slot.options) {
        const id = `${i}/${option.id}`;
        fields.add(id);
        const count = parseQuantity(draftValue(draft, id) || '0', 10);
        if (count === null || count > BigInt(slot.repeat ? slot.maximum : 1))
          return 'Enter a valid count for each candidate.';
        total += count;
      }
      if (total < BigInt(slot.minimum) || total > BigInt(slot.maximum))
        return `Choose ${slot.minimum}–${slot.maximum} item(s) for ${slot.label}.`;
      // A single serialized index takes at least one byte in the hosted command.
      if (total > 65536n) return 'Selection exceeds the command size limit.';
    }
    const entries = draft.values ?? [];
    if (new Set(entries.map(([id]) => id)).size !== entries.length || entries.some(([id]) => !fields.has(id)))
      return 'Selection is no longer in the supplied candidates.';
    return null;
  }
  if (control.kind === 'quantity')
    return parseQuantity(draft.quantity, control.maxDigits) === null
      ? `Enter a canonical nonnegative integer, at most ${control.maxDigits} digits.`
      : null;
  if (control.kind === 'mapping' || control.kind === 'quantities') {
    const fields = control.kind === 'mapping' ? control.rows : control.fields;
    const values = draft.values ?? [];
    if (new Set(fields.map((field) => field.id)).size !== fields.length) return 'Candidate identity is ambiguous.';
    if (
      new Set(values.map(([id]) => id)).size !== values.length ||
      values.some(([id]) => !fields.some((field) => field.id === id))
    )
      return 'Selection is no longer in the supplied candidates.';
    if (control.kind === 'mapping') {
      for (const row of control.rows) {
        if (new Set(row.options.map((option) => option.id)).size !== row.options.length)
          return 'Candidate identity is ambiguous.';
        if (!row.options.some((option) => option.id === draftValue(draft, row.id)))
          return 'Choose an option for each item, including any explicit decision to leave it unused.';
      }
    } else {
      let total = 0n;
      for (const field of control.fields) {
        const value = parseQuantity(draftValue(draft, field.id), control.maxDigits);
        if (value === null)
          return `Enter a canonical nonnegative integer for each item, at most ${control.maxDigits} digits.`;
        if (field.maximum !== undefined && value > BigInt(field.maximum))
          return 'An amount exceeds the supplied available quantity.';
        total += value;
      }
      if (control.total !== undefined && total !== BigInt(control.total))
        return `Assign exactly the supplied total of ${control.total}.`;
    }
    return null;
  }
  if (new Set(control.options.map((option) => option.id)).size !== control.options.length)
    return 'Candidate identity is ambiguous.';
  if (
    new Set(draft.selected).size !== draft.selected.length ||
    draft.selected.some((id) => !control.options.some((option) => option.id === id))
  )
    return 'Selection is no longer in the supplied candidates.';
  const minimum = control.kind === 'choice' ? 1 : control.minimum;
  const maximum = control.kind === 'choice' ? 1 : control.maximum;
  if (!Number.isSafeInteger(minimum) || !Number.isSafeInteger(maximum) || minimum < 0 || maximum < minimum)
    return 'Supplied count is unavailable.';
  return draft.selected.length < minimum || draft.selected.length > maximum
    ? `Choose ${minimum === maximum ? minimum : `${minimum}–${maximum}`} item(s).`
    : null;
}
export function toggleDraft(control: LocalControl, draft: LocalDraft, id: string): LocalDraft {
  if (
    control.kind === 'counts' ||
    control.kind === 'quantity' ||
    control.kind === 'mapping' ||
    control.kind === 'quantities' ||
    !control.options.some((option) => option.id === id)
  )
    return draft;
  if (control.kind === 'choice') return { ...draft, selected: [id] };
  if (draft.selected.includes(id)) return { ...draft, selected: draft.selected.filter((value) => value !== id) };
  return draft.selected.length < control.maximum ? { ...draft, selected: [...draft.selected, id] } : draft;
}
export function draftValue(draft: LocalDraft, id: string): string {
  return draft.values?.find(([key]) => key === id)?.[1] ?? '';
}
export function setDraftValue(draft: LocalDraft, id: string, value: string): LocalDraft {
  return { ...draft, values: [...(draft.values ?? []).filter(([key]) => key !== id), [id, value]] };
}
export function reorderDraft(draft: LocalDraft, id: string, offset: -1 | 1): LocalDraft {
  const from = draft.selected.indexOf(id),
    to = from + offset;
  if (from < 0 || to < 0 || to >= draft.selected.length) return draft;
  const selected = [...draft.selected];
  selected.splice(from, 1);
  selected.splice(to, 0, id);
  return { ...draft, selected };
}

/** Returns only generated responses for explicitly implemented request branches.
 * No default values, guesses, implicit pass, or side effects. */
function answerFor(control: LocalControl, draft: LocalDraft): Answer {
  const { shape } = control;
  switch (shape.shape) {
    case 'pick':
      return {
        answer: 'pick',
        picks:
          control.kind === 'counts'
            ? control.slots.map((slot, i) =>
                slot.options.flatMap((option) => {
                  const count = Number(draftValue(draft, `${i}/${option.id}`) || '0');
                  return Array<number>(count).fill(Number(option.id));
                }),
              )
            : control.kind === 'mapping'
              ? control.rows.map((row) => {
                  const id = draftValue(draft, row.id);
                  return id === 'unused' ? [] : [Number(id)];
                })
              : [draft.selected.map(Number)],
      };
    case 'number':
      return { answer: 'number', value: draft.quantity };
    case 'order':
      return { answer: 'order', order: draft.selected.map(Number) };
    case 'distribute':
      return { answer: 'distribute', amounts: shape.buckets.map((_, i) => draftValue(draft, String(i))) };
    case 'pay': {
      if (shape.step.step === 'spend')
        return {
          answer: 'pay',
          pay: { step: 'spend', unrestricted: Object.fromEntries((draft.values ?? []).filter(([, v]) => v !== '0')) },
        };
      const id = draft.selected[0]!;
      return {
        answer: 'pay',
        pay:
          id === 'finish' || id === 'finish-undo'
            ? { step: 'finish', undo_on_failure: id === 'finish-undo' }
            : { step: 'activate', source: Number(id) },
      };
    }
  }
}
export function draftError(control: LocalControl, draft: LocalDraft): string | null {
  const issue = fieldError(control, draft);
  if (issue) return issue;
  const { shape } = control;
  const answer = answerFor(control, draft);
  if (shape.shape === 'pick' && answer.answer === 'pick') {
    for (const [i, slot] of shape.slots.entries()) {
      const picked = answer.picks[i]!;
      if (
        picked.length < slot.min ||
        picked.length > slot.max ||
        picked.some((c) => !slot.allowed.includes(c)) ||
        (!slot.repeat && new Set(picked).size !== picked.length)
      )
        return 'Selection is outside the supplied slot.';
    }
    for (const rule of shape.rules) {
      if (rule.rule === 'distinct') {
        const indices = rule.slots.flatMap((i) => answer.picks[i] ?? []);
        if (new Set(indices).size !== indices.length) return 'Choose distinct candidates for these slots.';
      } else if (rule.rule === 'picked_at_least') {
        const indices = answer.picks.flat();
        if (
          rule.candidates.some((c) => {
            const n = indices.filter((i) => i === c).length;
            return n > 0 && n < rule.count;
          })
        )
          return `Choose each marked candidate at least ${rule.count} times or leave it unused.`;
      } else {
        const n = answer.picks.flat().length;
        if (n < rule.min || n > rule.max) return 'Selection is outside the supplied total.';
      }
    }
  }
  if (
    shape.shape === 'number' &&
    answer.answer === 'number' &&
    (BigInt(answer.value) < BigInt(shape.min) || (shape.max != null && BigInt(answer.value) > BigInt(shape.max)))
  )
    return 'Number is outside the supplied range.';
  if (shape.shape === 'distribute' && answer.answer === 'distribute') {
    if (shape.buckets.some((b, i) => BigInt(answer.amounts[i]!) < BigInt(b.min)))
      return 'An allocation is below its minimum.';
    if (
      shape.gates.some(
        (g) =>
          BigInt(answer.amounts[g.bucket]!) > 0n &&
          g.requires.some(([i, min]) => BigInt(answer.amounts[i]!) < BigInt(min)),
      )
    )
      return 'Meet the required allocations first.';
  }
  if (shape.shape === 'pay' && answer.answer === 'pay' && answer.pay.step === 'spend') {
    const { unrestricted } = answer.pay;
    for (const color of new Set(shape.cost.colored)) {
      if (BigInt(unrestricted[color] ?? '0') < BigInt(shape.cost.colored.filter((c) => c === color).length))
        return `Spend the required ${color} mana.`;
    }
  }
  return null;
}
export function completeDecision(
  request: Decision,
  labels: readonly ObjectLabel[],
  draft: LocalDraft,
  context?: DecisionContext | null,
): Answer | null {
  const surface = decisionSurface(request, labels, context);
  if (surface.status !== 'supported' || draftError(surface.control, draft)) return null;
  return answerFor(surface.control, draft);
}
