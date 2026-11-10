// The shared command boundary: retry identity, concurrency, atomic execution,
// one agent attribution record, and a compact result used by UI and MCP.
import { createHash } from "node:crypto";
import type { Database } from "./sqlite.ts";
import { requireActor, type Actor } from "./actor.ts";
import { withTransaction } from "./db.ts";
import { batchActivity } from "./activity.ts";
import { ServiceError } from "./errors.ts";
import { getDeck, requireDeck, requireRevision } from "./deck/service.ts";
import { checkDeck, type Finding } from "./deck/checks.ts";
import { withReadContext } from "./read-context.ts";
import { commandResources, type Resource } from "./changes.ts";

export interface CommandMeta {
  operation: string;
  replayed: boolean;
  resources: Resource[];
  global_resources: Resource[];
  deck_ids: number[];
  decks: Array<{ id: number; revision: number | null; edit_version: number | null; deleted: boolean;
    computed?: { card_count: number; delta_to_100: number; pending: ReturnType<typeof getDeck>["computed"]["pending"] };
    delta?: { card_count: number };
    warnings?: Finding[];
  }>;
  activity_ids: number[];
}
export interface CommandOptions {
  operation: string;
  input: unknown;
  idempotency_key?: string;
  expected_revision?: number;
  expected_edit_version?: number;
  /** Resolve after checking receipts so retrying a delete/rename still works. */
  deck?: () => number | undefined;
}
const digest = (text: string) => createHash("sha256").update(text).digest("hex");
function canonical(value: any): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value && typeof value === "object") return `{${Object.keys(value).filter(k => value[k] !== undefined).sort().map(k => `${JSON.stringify(k)}:${canonical(value[k])}`).join(",")}}`;
  return JSON.stringify(value) ?? "null";
}

export function executeCommand<T>(db: Database, actor: Actor, options: CommandOptions, execute: () => T): { value: T; command: CommandMeta } {
  return withReadContext(() => runCommand(db, actor, options, execute));
}
function runCommand<T>(db: Database, actor: Actor, options: CommandOptions, execute: () => T): { value: T; command: CommandMeta } {
  requireActor(actor);
  for (const [name, value] of [["expected_revision", options.expected_revision], ["expected_edit_version", options.expected_edit_version]] as const)
    if (value !== undefined && (!Number.isSafeInteger(value) || value < 0)) throw new ServiceError(`${name} must be a nonnegative integer`);
  const key = options.idempotency_key;
  if (key !== undefined && (typeof key !== "string" || !key.trim() || key.length > 128))
    throw new ServiceError("idempotency_key must contain 1–128 characters");
  const credential = actor.via === "ui" ? "ui" : actor.token_id != null ? `token:${actor.token_id}` : actor.oauth_client_id ? `oauth:${actor.oauth_client_id}` : null;
  if (key && !credential) throw new ServiceError("Idempotency requires a verified credential; attribution labels are not identity");
  const keyHash = key ? digest(key) : null;
  const inputHash = digest(canonical({ operation: options.operation, input: options.input, expected_revision: options.expected_revision, expected_edit_version: options.expected_edit_version }));
  // Cleanup is independent of command success: a refused expired retry must
  // not roll back the deletion of its obsolete response payload.
  if (keyHash) db.prepare("UPDATE command_receipts SET result_json=NULL WHERE user_id=? AND expires_at<=? AND result_json IS NOT NULL").run(actor.user_id, Date.now());
  return withTransaction(db, () => {
    if (keyHash) {
      // Keep compact tombstones: expired keys cannot silently execute again.
      const old = db.prepare("SELECT input_hash,result_json FROM command_receipts WHERE user_id=? AND credential=? AND key_hash=?").get(actor.user_id, credential!, keyHash) as { input_hash: string; result_json: string | null } | undefined;
      if (old) {
        if (old.input_hash !== inputHash) throw new ServiceError("This idempotency key was used for a different request. Use a new key for a new operation.", 409);
        if (!old.result_json) throw new ServiceError("This request's 24-hour replay window expired. Read current state before issuing a new request with a new key.", 409);
        const result = JSON.parse(old.result_json) as { value: T; command: CommandMeta };
        return { ...result, command: { ...result.command, replayed: true } };
      }
    }
    const deckId = options.deck?.();
    if (deckId !== undefined) {
      requireRevision(db, actor, deckId, options.expected_revision);
      const deck = requireDeck(db, actor, deckId);
      if (options.expected_edit_version !== undefined && options.expected_edit_version !== deck.edit_version)
        throw new ServiceError(`Deck ${deckId} changed since you read it (edit version ${deck.edit_version}). Read it again before editing.`, 409);
    } else if (options.expected_revision !== undefined || options.expected_edit_version !== undefined) {
      throw new ServiceError("A revision guard needs a deck target");
    }
    const resources = commandResources(options.operation);
    const before = deckId === undefined ? undefined : (db.prepare("SELECT COALESCE(SUM(quantity),0) n FROM deck_cards WHERE deck_id=? AND maybe=0 AND role<>'companion'").get(deckId) as { n: number }).n;
    const batch = batchActivity(db, actor, options.operation, execute, (value, affected) => {
      const deckIds = [...new Set([...(deckId === undefined ? [] : [deckId]), ...affected])];
      const stateId = (value as any)?.state?.deck?.id;
      if (typeof stateId === "number" && !deckIds.includes(stateId)) deckIds.push(stateId);
      const command: CommandMeta = { operation: options.operation, replayed: false,
        resources, deck_ids: deckIds, activity_ids: [],
        // Lineage/roster edits can change other decks' parent headers; explicit
        // ownership evidence is shared across the principal's decks.
        global_resources: resources.includes("focus") || options.operation === "record_ownership" ? ["deck"] : [],
        decks: deckIds.map(id => {
          const row = db.prepare("SELECT revision,edit_version FROM decks WHERE id=? AND user_id=?").get(id, actor.user_id) as { revision: number; edit_version: number } | undefined;
          const view = row && resources.includes("deck") ? getDeck(db, actor, id) : null;
          return { id, revision: row?.revision ?? null, edit_version: row?.edit_version ?? null, deleted: !row,
            ...(view ? { computed: { card_count: view.computed.card_count, delta_to_100: view.computed.delta_to_100, pending: view.computed.pending }, warnings: checkDeck(db, actor, id) } : {}),
            ...(id === deckId && before !== undefined ? { delta: { card_count: (view?.computed.card_count ?? (row ? before : 0)) - before } } : {}),
          };
        }),
      };
      return { value, command };
    });
    const result = batch.value;
    result.command.activity_ids = batch.activity_ids;
    if (keyHash) db.prepare("INSERT INTO command_receipts(user_id,credential,key_hash,input_hash,result_json,expires_at) VALUES(?,?,?,?,?,?)")
      .run(actor.user_id, credential!, keyHash, inputHash, JSON.stringify(result), Date.now() + 86_400_000);
    return result;
  });
}
