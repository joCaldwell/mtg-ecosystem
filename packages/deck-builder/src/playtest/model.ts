import { randomInt, randomUUID } from "node:crypto";
import { z } from "zod";
import { ServiceError } from "../errors.ts";

export const ZONES = [
  "library",
  "hand",
  "battlefield",
  "command",
  "companion",
  "graveyard",
  "exile",
  "stack",
] as const;
export type Zone = (typeof ZONES)[number];
export const ROWS = ["creatures", "permanents", "lands"] as const;
export type Row = (typeof ROWS)[number];
export type Mode = "commander" | "practice";
export interface CardFace {
  name: string;
  mana_cost?: string | null;
  type_line?: string;
  oracle_text?: string;
  power?: string | null;
  toughness?: string | null;
  loyalty?: string | null;
}
export interface TableCard extends CardFace {
  image_small?: string | null;
  image_normal?: string | null;
  image_art_crop?: string | null;
  oracle_id: string;
  faces: CardFace[];
}
export interface TableObject {
  id: string;
  oracle_id: string;
  token: boolean;
  role: "card" | "commander" | "companion";
  tapped: boolean;
  face: number;
  row: Row;
  counters: Record<string, number>;
  power: string | null;
  toughness: string | null;
  attached_to: string | null;
  cast_count: number;
}
export interface Frame {
  objects: Record<string, TableObject>;
  zones: Record<Zone, string[]>;
  life: number;
  turn: number;
  opening: {
    stage: "choosing" | "playing";
    mode: Mode;
    mulligans: number;
    bottom: string[];
  };
}
export interface PlaytestUi {
  selected: string[];
  pinned: string | null;
  viewer: Zone | null;
  draft: {
    text: string;
    oracle_id: string | null;
    request_id: string;
    deck_revision?: number;
  };
}
export interface PlaytestSession {
  format: 1;
  id: string;
  deck_id: number;
  deck_name: string;
  deck_revision: number;
  version: number;
  updated_at: string;
  cards: Record<string, TableCard>;
  frame: Frame;
  past: Frame[];
  future: Frame[];
  ui: PlaytestUi;
}
export interface PlaytestView {
  session: PlaytestSession;
  computed: {
    counts: Record<Zone, number>;
    bottom_required: number;
    can_keep: boolean;
    can_mulligan: boolean;
    can_undo: boolean;
    can_redo: boolean;
  };
}
const ids = z
  .array(z.string().min(1))
  .min(1)
  .max(1000)
  .refine((a) => new Set(a).size === a.length, "Select each object once");
const integer = z.number().int().min(-1000000).max(1000000);
export const actionSchema = z.discriminatedUnion("type", [
  z.object({
    type: z.literal("draw"),
    count: z.number().int().min(1).max(1000),
  }),
  z.object({ type: z.literal("shuffle") }),
  z.object({ type: z.literal("next-turn") }),
  z.object({ type: z.literal("untap-all") }),
  z.object({ type: z.literal("undo") }),
  z.object({ type: z.literal("redo") }),
  z.object({ type: z.literal("mulligan") }),
  z.object({ type: z.literal("keep") }),
  z.object({
    type: z.literal("mode"),
    mode: z.enum(["commander", "practice"]),
  }),
  z.object({
    type: z.literal("bottom"),
    ids: z
      .array(z.string())
      .max(7)
      .refine((a) => new Set(a).size === a.length),
  }),
  z.object({
    type: z.literal("move"),
    ids,
    zone: z.enum(ZONES),
    row: z.enum(ROWS).optional(),
    index: z.number().int().min(0).max(10000).optional(),
  }),
  z.object({ type: z.literal("tap"), ids, tapped: z.boolean() }),
  z.object({
    type: z.literal("counter"),
    id: z.string(),
    name: z.string().trim().min(1).max(60),
    delta: integer,
  }),
  z.object({ type: z.literal("life"), value: integer }),
  z.object({
    type: z.literal("edit"),
    id: z.string(),
    power: z.string().max(20).nullable().optional(),
    toughness: z.string().max(20).nullable().optional(),
    face: z.number().int().min(0).max(20).optional(),
    cast_count: z.number().int().min(0).max(10000).optional(),
  }),
  z.object({
    type: z.literal("attach"),
    id: z.string(),
    host: z.string().nullable(),
  }),
  z
    .object({
      type: z.literal("token"),
      oracle_id: z.string().optional(),
      copy_id: z.string().optional(),
    })
    .refine(
      (a) => !!a.oracle_id !== !!a.copy_id,
      "Choose a card or an object to copy",
    ),
  z.object({ type: z.literal("remove-token"), ids }),
]);
export type PlaytestAction = z.infer<typeof actionSchema>;
export const uiSchema = z.object({
  selected: z.array(z.string()).max(1000).optional(),
  pinned: z.string().nullable().optional(),
  viewer: z.enum(ZONES).nullable().optional(),
  draft: z
    .object({
      text: z.string().max(10000),
      oracle_id: z.string().nullable(),
      request_id: z.string().uuid(),
      deck_revision: z.number().int().min(0).optional(),
    })
    .optional(),
});
export type PlaytestUiPatch = z.infer<typeof uiSchema>;

export function shuffle<T>(items: T[]): T[] {
  const out = [...items];
  for (let i = out.length - 1; i > 0; i--) {
    const j = randomInt(i + 1);
    [out[i], out[j]] = [out[j], out[i]];
  }
  return out;
}
export function defaultRow(card: CardFace): Row {
  return /\bLand\b/.test(card.type_line ?? "")
    ? "lands"
    : /\bCreature\b/.test(card.type_line ?? "")
      ? "creatures"
      : "permanents";
}
export function makeObject(
  card: TableCard,
  role: TableObject["role"] = "card",
): TableObject {
  return {
    id: randomUUID(),
    oracle_id: card.oracle_id,
    role,
    token: false,
    tapped: false,
    face: 0,
    row: defaultRow(card.faces[0] ?? card),
    counters: {},
    power: null,
    toughness: null,
    attached_to: null,
    cast_count: 0,
  };
}
export function bottomRequired(frame: Frame): number {
  // CR 103.5, 103.5c (2026-09-25): seven cards each time, first multiplayer mulligan free.
  // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
  return frame.opening.mode === "practice"
    ? 0
    : Math.max(0, frame.opening.mulligans - 1);
}
export function view(session: PlaytestSession): PlaytestView {
  const f = session.frame,
    required = bottomRequired(f);
  return {
    session,
    computed: {
      counts: Object.fromEntries(
        ZONES.map((z) => [z, f.zones[z].length]),
      ) as Record<Zone, number>,
      bottom_required: required,
      can_keep:
        f.opening.stage === "choosing" && f.opening.bottom.length === required,
      can_mulligan:
        f.opening.stage === "choosing" &&
        (f.opening.mode === "practice" || f.opening.mulligans < 8),
      can_undo: session.past.length > 0,
      can_redo: session.future.length > 0,
    },
  };
}
export function zoneOf(frame: Frame, id: string): Zone {
  const zone = ZONES.find((z) => frame.zones[z].includes(id));
  if (!zone)
    throw new ServiceError("This object is no longer on the table", 404);
  return zone;
}
function object(frame: Frame, id: string): TableObject {
  const obj = frame.objects[id];
  if (!obj)
    throw new ServiceError("This object is no longer on the table", 404);
  return obj;
}
function move(
  frame: Frame,
  ids: string[],
  zone: Zone,
  index?: number,
  row?: Row,
) {
  for (const id of ids) {
    const obj = object(frame, id),
      source = zoneOf(frame, id);
    if (source === "battlefield" && zone !== "battlefield") {
      obj.attached_to = null;
      for (const other of Object.values(frame.objects))
        if (other.attached_to === id) other.attached_to = null;
    }
    if (row) obj.row = row;
  }
  for (const z of ZONES)
    frame.zones[z] = frame.zones[z].filter((id) => !ids.includes(id));
  frame.zones[zone].splice(
    index === undefined
      ? frame.zones[zone].length
      : Math.min(index, frame.zones[zone].length),
    0,
    ...ids,
  );
}
export function applyAction(
  session: PlaytestSession,
  action: PlaytestAction,
  tokenCard?: TableCard,
): void {
  if (action.type === "undo" || action.type === "redo") {
    const from = action.type === "undo" ? session.past : session.future;
    const to = action.type === "undo" ? session.future : session.past;
    const next = from.pop();
    if (!next) throw new ServiceError(`Nothing to ${action.type}`);
    to.push(session.frame);
    session.frame = next;
    return;
  }
  const before = structuredClone(session.frame),
    f = session.frame;
  const openingActions = ["mode", "mulligan", "bottom", "keep"];
  const reorderingHand =
    action.type === "move" &&
    action.zone === "hand" &&
    action.row === undefined &&
    action.ids.every((id) => f.zones.hand.includes(id));
  if (
    f.opening.stage === "choosing" &&
    !openingActions.includes(action.type) &&
    !reorderingHand &&
    !(
      action.type === "edit" &&
      action.face !== undefined &&
      action.power === undefined &&
      action.toughness === undefined &&
      action.cast_count === undefined
    )
  )
    throw new ServiceError("Keep an opening hand before playing the table");
  if (f.opening.stage === "playing" && openingActions.includes(action.type))
    throw new ServiceError("Start a new opening test to change mulligan mode");
  switch (action.type) {
    case "draw":
      move(f, f.zones.library.slice(0, action.count), "hand");
      break;
    case "shuffle":
      f.zones.library = shuffle(f.zones.library);
      break;
    case "next-turn":
      f.turn++;
      for (const id of f.zones.battlefield) f.objects[id].tapped = false;
      break;
    case "untap-all":
      for (const id of f.zones.battlefield) f.objects[id].tapped = false;
      break;
    case "life":
      f.life = action.value;
      break;
    case "move":
      move(f, action.ids, action.zone, action.index, action.row);
      break;
    case "tap":
      for (const id of action.ids) {
        if (zoneOf(f, id) !== "battlefield")
          throw new ServiceError("Only battlefield objects can be tapped");
        object(f, id).tapped = action.tapped;
      }
      break;
    case "counter": {
      const obj = object(f, action.id),
        n = (obj.counters[action.name] ?? 0) + action.delta;
      if (Math.abs(n) > 1000000)
        throw new ServiceError("Counter value is too large");
      if (n) obj.counters[action.name] = n;
      else delete obj.counters[action.name];
      break;
    }
    case "edit": {
      const obj = object(f, action.id);
      if (
        action.face !== undefined &&
        action.face >= Math.max(1, session.cards[obj.oracle_id].faces.length)
      )
        throw new ServiceError("That card face does not exist");
      for (const key of ["power", "toughness", "face", "cast_count"] as const)
        if (action[key] !== undefined)
          Object.assign(obj, { [key]: action[key] });
      break;
    }
    case "attach": {
      const obj = object(f, action.id);
      if (zoneOf(f, action.id) !== "battlefield")
        throw new ServiceError("Move an attachment onto the battlefield first");
      if (action.host) {
        object(f, action.host);
        if (zoneOf(f, action.host) !== "battlefield")
          throw new ServiceError("Choose a battlefield host");
        const visited = new Set([action.id]);
        let host: string | null = action.host;
        while (host) {
          if (visited.has(host))
            throw new ServiceError("Attachments cannot form a cycle");
          visited.add(host);
          host = object(f, host).attached_to;
        }
      }
      obj.attached_to = action.host;
      break;
    }
    case "token": {
      let obj: TableObject;
      if (action.copy_id) {
        obj = {
          ...structuredClone(object(f, action.copy_id)),
          id: randomUUID(),
          role: "card",
          token: true,
          attached_to: null,
          cast_count: 0,
        };
      } else {
        if (!tokenCard)
          throw new ServiceError("Choose a card from the database");
        session.cards[tokenCard.oracle_id] = tokenCard;
        obj = { ...makeObject(tokenCard), token: true };
      }
      f.objects[obj.id] = obj;
      f.zones.battlefield.push(obj.id);
      break;
    }
    case "remove-token": {
      for (const id of action.ids)
        if (!object(f, id).token)
          throw new ServiceError("Only tokens can be removed from the table");
      move(f, action.ids, "exile");
      f.zones.exile = f.zones.exile.filter((id) => !action.ids.includes(id));
      for (const id of action.ids) delete f.objects[id];
      break;
    }
    case "bottom": {
      if (
        action.ids.length > bottomRequired(f) ||
        action.ids.some((id) => !f.zones.hand.includes(id))
      )
        throw new ServiceError(
          "Choose the required bottom cards from this hand",
        );
      f.opening.bottom = action.ids;
      break;
    }
    case "keep": {
      if (!view(session).computed.can_keep)
        throw new ServiceError(
          "Choose all required bottom cards before keeping",
        );
      move(f, f.opening.bottom, "library");
      f.opening.bottom = [];
      f.opening.stage = "playing";
      f.turn = 1;
      break;
    }
    case "mode":
    case "mulligan": {
      if (action.type === "mulligan" && !view(session).computed.can_mulligan)
        throw new ServiceError("The mulligan limit is reached; keep this hand");
      move(f, [...f.zones.hand], "library");
      f.zones.library = shuffle(f.zones.library);
      f.opening.mode = action.type === "mode" ? action.mode : f.opening.mode;
      f.opening.mulligans =
        action.type === "mode" ? 0 : f.opening.mulligans + 1;
      f.opening.bottom = [];
      move(f, f.zones.library.slice(0, 7), "hand");
      break;
    }
  }
  session.past.push(before);
  if (session.past.length > 100) session.past.shift();
  session.future = [];
}

const faceSchema = z.object({
  name: z.string(),
  mana_cost: z.string().nullable().optional(),
  type_line: z.string().optional(),
  oracle_text: z.string().optional(),
  power: z.string().nullable().optional(),
  toughness: z.string().nullable().optional(),
  loyalty: z.string().nullable().optional(),
});
const objectSchema = z.object({
  id: z.string(),
  oracle_id: z.string(),
  token: z.boolean(),
  role: z.enum(["card", "commander", "companion"]),
  tapped: z.boolean(),
  face: z.number().int().min(0),
  row: z.enum(ROWS),
  counters: z.record(z.string(), integer),
  power: z.string().nullable(),
  toughness: z.string().nullable(),
  attached_to: z.string().nullable(),
  cast_count: z.number().int().min(0),
});
const frameSchema = z.object({
  objects: z.record(z.string(), objectSchema),
  zones: z.object({
    library: z.array(z.string()),
    hand: z.array(z.string()),
    battlefield: z.array(z.string()),
    command: z.array(z.string()),
    companion: z.array(z.string()),
    graveyard: z.array(z.string()),
    exile: z.array(z.string()),
    stack: z.array(z.string()),
  }),
  life: integer,
  turn: z.number().int().min(0),
  opening: z.object({
    stage: z.enum(["choosing", "playing"]),
    mode: z.enum(["commander", "practice"]),
    mulligans: z.number().int().min(0),
    bottom: z.array(z.string()).max(7),
  }),
});
export const sessionSchema = z.object({
  format: z.literal(1),
  id: z.string().uuid(),
  deck_id: z.number().int(),
  deck_name: z.string(),
  deck_revision: z.number().int().min(0),
  version: z.number().int().min(0),
  updated_at: z.string(),
  cards: z.record(
    z.string(),
    faceSchema.extend({ oracle_id: z.string(), faces: z.array(faceSchema) }),
  ),
  frame: frameSchema,
  past: z.array(frameSchema).max(100),
  future: z.array(frameSchema).max(100),
  ui: uiSchema.required(),
});
