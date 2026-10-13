// The solo table owns a private membership/card-fact snapshot. It never writes
// deck_cards, and its undo history contains game frames only (design brief).
import { randomUUID } from "node:crypto";
import type { DatabaseSync } from "node:sqlite";
import { z } from "zod";
import { type Actor, requireOwner } from "../actor.ts";
import { CARD_COLUMNS } from "../cards.ts";
import { withTransaction } from "../db.ts";
import { getDeck, requireDeck } from "../deck/service.ts";
import { ServiceError } from "../errors.ts";
import { getMemory, remember } from "../memory.ts";
import {
  applyAction,
  actionSchema,
  makeObject,
  shuffle,
  uiSchema,
  view,
  ZONES,
  sessionSchema,
} from "./model.ts";
import type { Frame, Mode, PlaytestSession, TableCard } from "./model.ts";
export type {
  PlaytestAction,
  PlaytestUiPatch,
  PlaytestView,
  TableCard,
  TableObject,
  CardFace,
  Zone,
  Row,
  Mode,
} from "./model.ts";

export function tableCard(db: DatabaseSync, oracleId: string): TableCard {
  const c = db
    .prepare(
      `SELECT ${CARD_COLUMNS}, faces_json FROM cards WHERE oracle_id = ?`,
    )
    .get(oracleId) as (TableCard & { faces_json: string | null }) | undefined;
  if (!c) throw new ServiceError("This card is not in the database", 404);
  return {
    oracle_id: c.oracle_id,
    name: c.name,
    mana_cost: c.mana_cost,
    type_line: c.type_line,
    oracle_text: c.oracle_text,
    power: c.power,
    toughness: c.toughness,
    loyalty: c.loyalty,
    faces: c.faces_json ? JSON.parse(c.faces_json) : [],
  };
}
function read(db: DatabaseSync, deckId: number): PlaytestSession | null {
  requireDeck(db, deckId);
  const row = db
    .prepare("SELECT payload_json FROM playtest_sessions WHERE deck_id = ?")
    .get(deckId) as { payload_json: string } | undefined;
  if (!row) return null;
  try {
    const s = JSON.parse(row.payload_json) as PlaytestSession;
    sessionSchema.parse(s);
    if (
      s.format !== 1 ||
      s.deck_id !== deckId ||
      !s.id ||
      !Number.isInteger(s.version) ||
      !s.cards ||
      !s.ui ||
      !Array.isArray(s.past) ||
      !Array.isArray(s.future)
    )
      throw Error();
    for (const f of [s.frame, ...s.past, ...s.future]) {
      if (
        !f.objects ||
        !Number.isInteger(f.life) ||
        !Number.isInteger(f.turn) ||
        !f.opening ||
        !["choosing", "playing"].includes(f.opening.stage)
      )
        throw Error();
      const all = ZONES.flatMap((z) => {
        if (!Array.isArray(f.zones[z])) throw Error();
        return f.zones[z];
      });
      if (
        new Set(all).size !== all.length ||
        all.length !== Object.keys(f.objects).length
      )
        throw Error();
      for (const id of all) {
        const o = f.objects[id];
        if (
          !o ||
          o.id !== id ||
          !s.cards[o.oracle_id] ||
          (o.attached_to && !f.zones.battlefield.includes(o.attached_to))
        )
          throw Error();
      }
    }
    uiSchema.parse(s.ui);
    return s;
  } catch {
    throw new ServiceError(
      "The saved playtest cannot be read. It was preserved; explicitly start a new session to replace it.",
      422,
    );
  }
}
function write(db: DatabaseSync, session: PlaytestSession) {
  session.updated_at = new Date().toISOString();
  db.prepare(
    `INSERT INTO playtest_sessions (deck_id, payload_json) VALUES (?, ?)
    ON CONFLICT(deck_id) DO UPDATE SET payload_json = excluded.payload_json`,
  ).run(session.deck_id, JSON.stringify(session));
  return view(session);
}
export function getPlaytest(db: DatabaseSync, actor: Actor, deckId: number) {
  requireOwner(actor, "Read the owner's playtest");
  const session = read(db, deckId);
  return session ? view(session) : null;
}
const startSchema = z.object({
  mode: z.enum(["commander", "practice"]),
  replace: z.boolean().default(false),
  session_id: z.string().optional(),
  expected_version: z.number().int().optional(),
});
export function startPlaytest(
  db: DatabaseSync,
  actor: Actor,
  deckId: number,
  input: unknown,
) {
  requireOwner(actor, "Start a playtest");
  const parsed = startSchema.safeParse(input);
  if (!parsed.success)
    throw new ServiceError(
      parsed.error.issues.map((i) => i.message).join("; "),
    );
  return withTransaction(db, () => {
    const existingRow = db
      .prepare("SELECT payload_json FROM playtest_sessions WHERE deck_id = ?")
      .get(deckId) as { payload_json: string } | undefined;
    if (existingRow && !parsed.data.replace)
      throw new ServiceError(
        "A session already exists. Resume it or confirm replacing it.",
        409,
      );
    let previous: PlaytestSession | null = null;
    if (existingRow) {
      try {
        previous = read(db, deckId);
      } catch (error) {
        if (!(error instanceof ServiceError) || error.status !== 422)
          throw error;
      }
      if (previous)
        checkVersion(
          previous,
          parsed.data.session_id,
          parsed.data.expected_version,
        );
    }
    const deck = getDeck(db, deckId),
      cards: Record<string, TableCard> = {};
    const frame: Frame = {
      objects: {},
      zones: Object.fromEntries(
        ZONES.map((z) => [z, [] as string[]]),
      ) as Frame["zones"],
      life: 40,
      turn: 0,
      opening: {
        stage: "choosing",
        mode: parsed.data.mode as Mode,
        mulligans: 0,
        bottom: [],
      },
    };
    for (const entry of deck.cards.filter((c) => !c.maybe)) {
      const card = tableCard(db, entry.oracle_id);
      cards[card.oracle_id] = card;
      for (let i = 0; i < entry.quantity; i++) {
        const obj = makeObject(card, entry.role);
        frame.objects[obj.id] = obj;
        frame.zones[
          entry.role === "commander"
            ? "command"
            : entry.role === "companion"
              ? "companion"
              : "library"
        ].push(obj.id);
      }
    }
    if (frame.zones.library.length < 7)
      throw new ServiceError(
        "Add at least seven non-commander cards before testing opening hands",
      );
    frame.zones.library = shuffle(frame.zones.library);
    frame.zones.hand = frame.zones.library.splice(0, 7);
    const session: PlaytestSession = {
      format: 1,
      id: randomUUID(),
      deck_id: deckId,
      deck_name: deck.deck.name,
      deck_revision: deck.deck.revision,
      version: 0,
      updated_at: "",
      cards,
      frame,
      past: [],
      future: [],
      ui: {
        selected: [],
        pinned: null,
        viewer: null,
        draft: {
          text: "",
          oracle_id: null,
          request_id: randomUUID(),
          deck_revision: deck.deck.revision,
        },
      },
    };
    if (previous?.ui.draft.text) {
      session.ui.draft = {
        ...previous.ui.draft,
        deck_revision:
          previous.ui.draft.deck_revision ?? previous.deck_revision,
      };
      const oid = session.ui.draft.oracle_id;
      if (oid && previous.cards[oid])
        session.cards[oid] ??= previous.cards[oid];
    }
    return write(db, session);
  });
}
function checkVersion(
  s: PlaytestSession,
  sessionId: unknown,
  expected: unknown,
) {
  if (sessionId !== s.id || expected !== s.version)
    throw new ServiceError(
      "This table changed in another window. Reload the saved session before continuing.",
      409,
    );
}
export function changePlaytest(
  db: DatabaseSync,
  actor: Actor,
  deckId: number,
  input: { session_id?: unknown; expected_version?: unknown; action?: unknown },
) {
  requireOwner(actor, "Change a playtest");
  const parsed = actionSchema.safeParse(input?.action);
  if (!parsed.success)
    throw new ServiceError(
      parsed.error.issues.map((i) => i.message).join("; "),
    );
  return withTransaction(db, () => {
    const s = read(db, deckId);
    if (!s) throw new ServiceError("Start a session first", 404);
    checkVersion(s, input.session_id, input.expected_version);
    const action = parsed.data;
    applyAction(
      s,
      action,
      action.type === "token" && action.oracle_id
        ? tableCard(db, action.oracle_id)
        : undefined,
    );
    s.ui.selected = s.ui.selected.filter((id) => !!s.frame.objects[id]);
    if (s.ui.pinned && !s.frame.objects[s.ui.pinned]) s.ui.pinned = null;
    s.version++;
    return write(db, s);
  });
}
export function savePlaytestUi(
  db: DatabaseSync,
  actor: Actor,
  deckId: number,
  input: { session_id?: unknown; expected_version?: unknown; ui?: unknown },
) {
  requireOwner(actor, "Save playtest selection and draft");
  const parsed = uiSchema.safeParse(input?.ui);
  if (!parsed.success)
    throw new ServiceError("Invalid playtest selection or draft");
  return withTransaction(db, () => {
    const s = read(db, deckId);
    if (!s) throw new ServiceError("Start a session first", 404);
    checkVersion(s, input.session_id, input.expected_version);
    const patch = parsed.data;
    for (const id of [
      ...(patch.selected ?? []),
      ...(patch.pinned ? [patch.pinned] : []),
    ])
      if (!s.frame.objects[id])
        throw new ServiceError("That object is no longer on the table");
    if (patch.draft?.oracle_id && !s.cards[patch.draft.oracle_id])
      throw new ServiceError(
        "Choose a card in this session for the observation",
      );
    if (patch.draft) {
      patch.draft.deck_revision = patch.draft.text
        ? (patch.draft.deck_revision ??
          (s.ui.draft.text ? s.ui.draft.deck_revision : undefined) ??
          s.deck_revision)
        : s.deck_revision;
      if (patch.draft.deck_revision > requireDeck(db, deckId).revision)
        throw new ServiceError(
          "The observation revision is newer than this deck",
        );
    }
    Object.assign(s.ui, patch);
    s.version++;
    return write(db, s);
  });
}
const observationSchema = z.object({
  session_id: z.string(),
  request_id: z.string().uuid(),
  text: z.string().min(1),
  oracle_id: z.string().nullable(),
  deck_revision: z.number().int().min(0).optional(),
});
export function saveObservation(
  db: DatabaseSync,
  actor: Actor,
  deckId: number,
  input: unknown,
) {
  requireOwner(actor, "Save a playtest observation");
  const parsed = observationSchema.safeParse(input);
  if (!parsed.success)
    throw new ServiceError("An observation needs text and a session");
  const data = parsed.data;
  return withTransaction(db, () => {
    // Idempotence lets a timed-out save be retried without writing a duplicate.
    const prior = db
      .prepare(
        "SELECT memory_id, input_json FROM playtest_observations WHERE deck_id = ? AND request_id = ?",
      )
      .get(deckId, data.request_id) as
      | { memory_id: number; input_json: string }
      | undefined;
    if (prior) {
      if (prior.input_json !== JSON.stringify(data))
        throw new ServiceError(
          "This save already has different text; start a new observation",
          409,
        );
      return getMemory(db, prior.memory_id);
    }
    const s = read(db, deckId);
    if (!s || data.session_id !== s.id)
      throw new ServiceError(
        "The playtest was replaced. Your text is still in the editor; reload before saving.",
        409,
      );
    if (data.oracle_id && !s.cards[data.oracle_id])
      throw new ServiceError("That card is not in the tested session");
    const memory = remember(db, actor, {
      text: data.text,
      scope: "deck",
      deck_id: deckId,
      cards: data.oracle_id ? [data.oracle_id] : [],
      deck_revision:
        data.deck_revision ??
        (s.ui.draft.request_id === data.request_id
          ? s.ui.draft.deck_revision
          : undefined) ??
        s.deck_revision,
    }).memory;
    db.prepare(
      "INSERT INTO playtest_observations (deck_id, request_id, memory_id, input_json) VALUES (?, ?, ?, ?)",
    ).run(deckId, data.request_id, memory.id, JSON.stringify(data));
    return memory;
  });
}
