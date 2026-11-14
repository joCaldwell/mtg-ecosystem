import type {
  TableCard,
  TableObject,
  Zone,
  TableRow,
  PlaytestView,
} from "../api/client.ts";
export type { TableCard, TableObject, Zone, TableRow };
export const TABLE_ZONES: Zone[] = [
  "library",
  "command",
  "companion",
  "graveyard",
  "exile",
  "stack",
  "hand",
  "battlefield",
];
export const ZONE_LABELS: Record<Zone, string> = {
  library: "Library",
  command: "Command zone",
  companion: "Companion",
  graveyard: "Graveyard",
  exile: "Exile",
  stack: "Stack",
  hand: "Hand",
  battlefield: "Battlefield",
};
export const ROW_LABELS: Record<TableRow, string> = {
  creatures: "Creatures",
  permanents: "Other permanents",
  lands: "Lands",
};
export function faceOf(card: TableCard, face = 0) {
  return card.faces[face] ?? card;
}
export function selectedAfterClick(
  selected: string[],
  id: string,
  multiple: boolean,
) {
  return multiple
    ? selected.includes(id)
      ? selected.filter((x) => x !== id)
      : [...selected, id]
    : [id];
}
export function objectZone(view: PlaytestView, id: string) {
  return TABLE_ZONES.find((z) => view.session.frame.zones[z].includes(id));
}
export function orderedTop(view: PlaytestView, count: number) {
  return view.session.frame.zones.library.slice(0, Math.max(0, count));
}
/** Presentation state supplied by any host, independent of manual sessions
 * and the future engine's authoritative object/action protocol. */
export interface DisplayObject {
  id: string;
  face?: number;
  tapped?: boolean;
  token?: boolean;
  counters?: Record<string, number>;
  power?: string | null;
  toughness?: string | null;
  attached_to?: string | null;
}
export type CardObservation =
  | {
      visibility: "known";
      card: TableCard;
      object?: DisplayObject;
      owner?: string;
      controller?: string;
      attachedTo?: string;
      attachments?: string[];
    }
  | { visibility: "hidden"; label?: string };

export function observeObject(view: PlaytestView, id: string): CardObservation {
  const { objects, zones } = view.session.frame,
    obj = objects[id],
    card = view.session.cards[obj.oracle_id];
  const name = (oid: string) =>
    faceOf(view.session.cards[objects[oid].oracle_id], objects[oid].face).name;
  return {
    visibility: "known",
    card,
    object: obj,
    attachedTo: obj.attached_to ? name(obj.attached_to) : undefined,
    attachments: zones.battlefield
      .filter((oid) => objects[oid].attached_to === id)
      .map(name),
  };
}
