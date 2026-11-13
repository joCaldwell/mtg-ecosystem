/** Local draft bookkeeping, parameterized by generated token/handle types. The
 * caller supplies candidates/constraints; membership is not joint rules legality. */
export interface DraftBinding<Token> {
  readonly match: string;
  readonly seat: string;
  readonly token: Token;
}
export interface SelectionDraft<Token, Handle> {
  readonly binding: DraftBinding<Token>;
  readonly selected: readonly Handle[];
}

export function sameBinding<Token>(a: DraftBinding<Token>, b: DraftBinding<Token>): boolean {
  return a.match === b.match && a.seat === b.seat && Object.is(a.token, b.token);
}

/** Token changes discard intent. Handle disappearance removes only that handle;
 * names/Oracle IDs cannot transfer selection to a different incarnation. */
export function revalidateSelection<Token, Handle>(
  draft: SelectionDraft<Token, Handle>, binding: DraftBinding<Token>, candidates: readonly Handle[],
): SelectionDraft<Token, Handle> | null {
  if (!sameBinding(draft.binding, binding)) return null;
  const allowed = new Set(candidates);
  return Object.freeze({ binding, selected: Object.freeze(draft.selected.filter(handle => allowed.has(handle))) });
}

export function toggleSelection<Token, Handle>(
  draft: SelectionDraft<Token, Handle>, handle: Handle, candidates: readonly Handle[], maximum: number,
): SelectionDraft<Token, Handle> {
  if (!Number.isSafeInteger(maximum) || maximum < 0) throw new Error('Invalid supplied selection bound');
  if (!candidates.includes(handle)) return draft;
  const selected = draft.selected.includes(handle)
    ? draft.selected.filter(value => value !== handle)
    : draft.selected.length < maximum ? [...draft.selected, handle] : draft.selected;
  return selected === draft.selected ? draft : Object.freeze({ ...draft, selected: Object.freeze(selected) });
}

export function moveSelection<Token, Handle>(
  draft: SelectionDraft<Token, Handle>, from: number, to: number,
): SelectionDraft<Token, Handle> {
  if (!Number.isInteger(from) || !Number.isInteger(to) || from < 0 || to < 0 || from >= draft.selected.length || to >= draft.selected.length) return draft;
  const selected = [...draft.selected];
  const [value] = selected.splice(from, 1);
  selected.splice(to, 0, value as Handle);
  return Object.freeze({ ...draft, selected: Object.freeze(selected) });
}
