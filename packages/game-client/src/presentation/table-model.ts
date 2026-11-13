/** UI display models, NOT protocol definitions. Only source-qualified permitted
 * projections may populate these fields. No legality/visibility is computed here. */
export interface CardView {
  readonly inspectionId: string;
  readonly name: string | null;
  readonly printedType: string | null;
  readonly printedMana: string | null;
  readonly oracleText: string | null;
  readonly sourceIdentity: string;
  readonly zoneLabel: string;
  readonly currentFacts: readonly { readonly label: string; readonly value: string }[];
}
export interface SeatView {
  readonly label: string;
  readonly isViewer: boolean;
  readonly life: string | null;
  readonly handCount: string | null;
  readonly libraryCount: string | null;
  readonly battlefield: readonly CardView[];
}
export interface TableView {
  readonly title: string;
  readonly sourceLabel: string;
  readonly seats: readonly SeatView[];
  readonly hand: readonly CardView[];
  readonly commandZone: readonly CardView[];
  readonly stack: readonly CardView[];
  readonly turn: string | null;
  readonly phase: string | null;
  readonly priority: string | null;
  readonly decision: { readonly title: string; readonly description: string; readonly details: readonly string[] } | null;
}

export function cardName(card: CardView): string { return card.name ?? 'Unidentified card'; }

export function inspectableCards(table: TableView): readonly CardView[] {
  return [...table.hand, ...table.commandZone, ...table.stack, ...table.seats.flatMap(seat => seat.battlefield)];
}
