// Single owner of the chat_messages table. The transcript is an append-only
// session log: rows are inserted here, read back here, and the only other
// thing that ever happens to one is markCompacted() stamping it out of
// resident context (spec §11) — never a DELETE.

import type { DatabaseSync } from "node:sqlite";
import type { ChatMessage } from "./llm.ts";

// What a persisted row holds beyond the provider shape: proposal_id links a
// tool message to the proposal it created, so the chat can render the
// proposal itself rather than a line saying one happened.
export type StoredChatMessage = ChatMessage & { proposal_id?: number };

// `extra` is stored alongside the message but deliberately not part of the
// ChatMessage the provider later sees — the row on disk keeps the link the
// UI needs, the model gets exactly the fields it knows about.
export function appendMessage(
  db: DatabaseSync,
  deckId: number,
  msg: ChatMessage,
  extra?: Record<string, unknown>,
): void {
  db.prepare("INSERT INTO chat_messages (deck_id, role, content_json) VALUES (?, ?, ?)").run(
    deckId,
    msg.role,
    JSON.stringify(extra ? { ...msg, ...extra } : msg),
  );
}

// Messages still in resident context — compacted ones stay on disk but are
// excluded here (spec §11).
export function residentMessages(
  db: DatabaseSync,
  deckId: number,
): Array<{ id: number; message: StoredChatMessage }> {
  return (
    db
      .prepare(
        "SELECT id, content_json FROM chat_messages WHERE deck_id = ? AND compacted_at IS NULL ORDER BY id",
      )
      .all(deckId) as unknown as Array<{ id: number; content_json: string }>
  ).map((r) => ({ id: r.id, message: JSON.parse(r.content_json) as StoredChatMessage }));
}

export interface ChatRow {
  id: number;
  created_at: string;
  compacted_at: string | null;
  message: StoredChatMessage;
}

// The whole transcript, compacted rows included — the UI shows those
// collapsed, so compaction is visibly non-destructive (spec §11).
export function allMessages(db: DatabaseSync, deckId: number): ChatRow[] {
  return (
    db
      .prepare(
        "SELECT id, content_json, compacted_at, created_at FROM chat_messages WHERE deck_id = ? ORDER BY id",
      )
      .all(deckId) as unknown as Array<{
      id: number;
      content_json: string;
      compacted_at: string | null;
      created_at: string;
    }>
  ).map((r) => ({
    id: r.id,
    created_at: r.created_at,
    compacted_at: r.compacted_at,
    message: JSON.parse(r.content_json) as StoredChatMessage,
  }));
}

// The compaction write (spec §11's hard boundary — see consolidate.ts for
// the full contract). An UPDATE stamping compacted_at, nothing else.
export function markCompacted(db: DatabaseSync, deckId: number, throughMessageId: number): void {
  db.prepare(
    "UPDATE chat_messages SET compacted_at = datetime('now') WHERE deck_id = ? AND id <= ? AND compacted_at IS NULL",
  ).run(deckId, throughMessageId);
}

export function countCompacted(db: DatabaseSync, deckId: number): number {
  const { n } = db
    .prepare(
      "SELECT COUNT(*) n FROM chat_messages WHERE deck_id = ? AND compacted_at IS NOT NULL",
    )
    .get(deckId) as { n: number };
  return n;
}
