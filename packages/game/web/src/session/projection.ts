/** Normalized local publications, never decoded wire data. An SDK adapter must
 * validate authorization, cursor order and generation before calling this helper. */
export interface Projection<View, Cursor, Lifecycle, Outcome> {
  readonly cursor: Cursor;
  readonly observation: View | null;
  readonly lifecycle: Lifecycle | null;
  readonly outcome: Outcome | null;
}
export type Publication<View, Cursor, Lifecycle, Outcome> =
  | { readonly kind: 'observation'; readonly cursor: Cursor; readonly value: View }
  | { readonly kind: 'lifecycle'; readonly cursor: Cursor; readonly value: Lifecycle }
  | { readonly kind: 'outcome'; readonly cursor: Cursor; readonly value: Outcome };

export function applyPublication<View, Cursor, Lifecycle, Outcome>(
  current: Projection<View, Cursor, Lifecycle, Outcome>,
  publication: Publication<View, Cursor, Lifecycle, Outcome>,
): Projection<View, Cursor, Lifecycle, Outcome> {
  switch (publication.kind) {
    case 'observation': return Object.freeze({ ...current, cursor: publication.cursor, observation: publication.value });
    case 'lifecycle': return Object.freeze({ ...current, cursor: publication.cursor, lifecycle: publication.value });
    case 'outcome': return Object.freeze({ ...current, cursor: publication.cursor, outcome: publication.value });
  }
}
