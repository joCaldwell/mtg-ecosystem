/** Public in-process application ports. No game runtime/SDK dependency. */
import type { IncomingMessage, ServerResponse } from "node:http";
import type { Database } from "./sqlite.ts";
import type { Actor } from "./actor.ts";
import { requireDeckOwner, requireUi } from "./actor.ts";
import { accountInfo } from "./accounts.ts";
import { databaseGate } from "./auth-database.ts";
import { withTransaction } from "./db.ts";
import { ServiceError } from "./errors.ts";
export { ServiceError } from "./errors.ts";
export type { Actor } from "./actor.ts";
export interface DeckCapture {
  deck_id: number;
  revision: number;
  cards: ReadonlyArray<{ oracle_id: string; role: "card" | "commander" | "companion"; quantity: number }>;
}
/** Shared connection and its existing reentrant reservation, never a second DB.
 * Consumers reserve SQL only; outbound I/O must occur after reservation release. */
export function applicationPorts(db: Database) {
  const authorizeHuman = (actor: Actor) => { requireUi(actor, "launch or resume games"); accountInfo(db, actor); };
  return {
    database: db,
    sql: <T>(work: () => T | Promise<T>): Promise<T> => databaseGate(db).run(work),
    transaction: <T>(work: () => T): T => withTransaction(db, work),
    authorizeHuman,
    captureOwnedDeck(actor: Actor, deckId: number, revision: number): DeckCapture | { state: "stale_revision"; current_revision: number } {
      authorizeHuman(actor);
      if (!Number.isSafeInteger(deckId) || deckId < 1 || !Number.isSafeInteger(revision) || revision < 0) throw new ServiceError("Deck and revision must be valid integers");
      const deck = requireDeckOwner(db, actor, deckId);
      if (deck.revision !== revision) return { state: "stale_revision", current_revision: deck.revision };
      const cards = db.prepare("SELECT oracle_id, role, quantity FROM deck_cards WHERE deck_id=? AND maybe=0 ORDER BY oracle_id").all(deckId) as unknown as DeckCapture["cards"];
      return { deck_id: deckId, revision, cards };
    },
  };
}
export type ApplicationPorts = ReturnType<typeof applicationPorts>;
export interface ApplicationRequest {
  req: IncomingMessage;
  res: ServerResponse;
  url: URL;
  actor: Actor;
  json(status: number, value: unknown): void;
  readJson(req: IncomingMessage): Promise<Record<string, any>>;
  readRequestBody(req: IncomingMessage): Promise<Buffer>;
}
/** Requests are authenticated and Origin/rate checked by the existing host,
 * then dispatched outside its handler-wide SQL reservation. */
export interface ApplicationExtension {
  matches(url: URL): boolean;
  handle(request: ApplicationRequest): Promise<void>;
  start(): void;
  close(): Promise<void>;
}
export type ApplicationFactory = (ports: ApplicationPorts) => ApplicationExtension;
