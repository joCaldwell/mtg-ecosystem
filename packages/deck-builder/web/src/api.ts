// Typed client. Wire types are imported from the server source (type-only,
// so they are erased at build and Vite never bundles server code) — a field
// change on the server fails this typecheck instead of failing at runtime.

import type { DeckState } from "../../src/state.ts";
import type { auditState, AuditRun, Finding, ReasoningSnapshot } from "../../src/deck/audit.ts";
import type { BriefView, EngineView } from "../../src/deck/brief.ts";
import type { LogEntry, HardFilter, CardNote } from "../../src/deck/log.ts";
import type { ExportResult, ImportDiff, PlaytestNote } from "../../src/deck/interop.ts";
import type { listDecks } from "../../src/deck/service.ts";
import type { SearchResult } from "../../src/search/index.ts";
import type { ConsolidationView } from "../../src/agent/consolidate.ts";
import type { MeterResult, MeterSegment } from "../../src/agent/meter.ts";
import type { ChatHistoryMessage } from "../../src/agent/agent.ts";

export type { DeckState, LogEntry, HardFilter, CardNote, ExportResult, ImportDiff, PlaytestNote, AuditRun, MeterSegment };
export type CardData = SearchResult;
export type DeckCard = DeckState["cards"][number];
export type Slot = DeckState["slots"][number];
export type Tag = DeckState["tags"][number];
export type Computed = DeckState["computed"];
export type SlotDelta = Computed["slot_deltas"][number];
export type Proposal = DeckState["proposals"][number];
export type ProposalItem = Proposal["items"][number];
export type BriefEdit = DeckState["brief_edits"][number];
export type Brief = BriefView;
export type Engine = EngineView;
export type AuditFinding = Finding;
export type ReasoningResult = ReasoningSnapshot;
export type AuditState = ReturnType<typeof auditState>;
export type ChatMsg = ChatHistoryMessage;
export type Consolidation = ConsolidationView;
export type ContextMeter = MeterResult;
export type DeckSummary = ReturnType<typeof listDecks>[number];

// Web-only: a draft proposal item being assembled in the UI. card_name is
// display sugar and is stripped before the POST.
export interface DraftItem {
  action: "add" | "cut";
  oracle_id: string;
  card_name: string;
  slot_id?: number | null;
  rationale: string;
  group_id?: string | null;
}

// The server's envelope rule: a response carrying a domain composite names
// it. The deck store's apply() dispatches whichever keys are present, so
// every mutation call site is `apply(await api.x(...))`.
export interface Envelope {
  state?: DeckState;
  audit?: AuditState;
  brief?: Brief;
  meter?: ContextMeter;
  consolidation?: Consolidation;
}

export class ApiError extends Error {}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(path, {
    method,
    headers: body !== undefined ? { "Content-Type": "application/json" } : undefined,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  const data = await res.json();
  if (!res.ok) throw new ApiError(data.error ?? `HTTP ${res.status}`);
  return data as T;
}

export const api = {
  listDecks: () => request<DeckSummary[]>("GET", "/api/decks"),
  createDeck: (name: string) => request<{ state: DeckState }>("POST", "/api/decks", { name }),
  getDeck: (id: number) => request<{ state: DeckState }>("GET", `/api/decks/${id}`),
  renameDeck: (id: number, name: string) =>
    request<{ state: DeckState }>("PATCH", `/api/decks/${id}`, { name }),
  deleteDeck: (id: number) => request<{ ok: true }>("DELETE", `/api/decks/${id}`),

  addCard: (deckId: number, oracleId: string, slotId?: number | null, role?: string) =>
    request<{ state: DeckState }>("POST", `/api/decks/${deckId}/cards`, {
      oracle_id: oracleId,
      slot_id: slotId ?? null,
      role,
    }),
  updateCard: (
    deckId: number,
    oracleId: string,
    patch: Partial<{
      slot_id: number | null;
      role: string;
      owned: boolean;
      quantity: number;
      tag_ids: number[];
    }>,
  ) => request<{ state: DeckState }>("PATCH", `/api/decks/${deckId}/cards/${oracleId}`, patch),
  removeCard: (deckId: number, oracleId: string) =>
    request<{ state: DeckState }>("DELETE", `/api/decks/${deckId}/cards/${oracleId}`),

  createSlot: (deckId: number, name: string, min: number | null, max: number | null) =>
    request<{ state: DeckState }>("POST", `/api/decks/${deckId}/slots`, {
      name,
      target_min: min,
      target_max: max,
    }),
  updateSlot: (
    deckId: number,
    slotId: number,
    patch: Partial<{ name: string; target_min: number | null; target_max: number | null }>,
  ) => request<{ state: DeckState }>("PATCH", `/api/decks/${deckId}/slots/${slotId}`, patch),
  deleteSlot: (deckId: number, slotId: number) =>
    request<{ state: DeckState }>("DELETE", `/api/decks/${deckId}/slots/${slotId}`),

  createTag: (deckId: number, name: string) =>
    request<{ state: DeckState }>("POST", `/api/decks/${deckId}/tags`, { name }),
  deleteTag: (deckId: number, tagId: number) =>
    request<{ state: DeckState }>("DELETE", `/api/decks/${deckId}/tags/${tagId}`),

  createProposal: (deckId: number, items: DraftItem[], note?: string) =>
    request<{ state: DeckState }>("POST", `/api/decks/${deckId}/proposals`, {
      items: items.map(({ card_name, ...i }) => i),
      note,
    }),
  acceptItem: (deckId: number, itemId: number) =>
    request<{ state: DeckState }>("POST", `/api/decks/${deckId}/items/${itemId}/accept`),
  rejectItem: (deckId: number, itemId: number, type: string, reason: string) =>
    request<{ state: DeckState }>("POST", `/api/decks/${deckId}/items/${itemId}/reject`, { type, reason }),
  undoDecision: (deckId: number, logId: number) =>
    request<{ state: DeckState }>("POST", `/api/decks/${deckId}/log/${logId}/undo`),
  removeHardFilter: (deckId: number, oracleId: string) =>
    request<{ state: DeckState }>("DELETE", `/api/decks/${deckId}/filters/${oracleId}`),
  addCardNote: (deckId: number, oracleId: string, note: string) =>
    request<{ state: DeckState }>("POST", `/api/decks/${deckId}/notes`, { oracle_id: oracleId, note }),

  getAudit: (deckId: number) => request<{ audit: AuditState }>("GET", `/api/decks/${deckId}/audit`),
  // Returns as soon as the run is open — the reasoning pass finishes on the
  // server, so this resolving is not the run being done. Poll getAudit.
  startAudit: (deckId: number, instructions: string) =>
    request<{ run_id: number; already_running: boolean; audit: AuditState }>(
      "POST",
      `/api/decks/${deckId}/audit`,
      { instructions },
    ),
  dismissAuditFinding: (deckId: number, key: string, type: string, reason: string) =>
    request<{ state: DeckState; audit: AuditState }>("POST", `/api/decks/${deckId}/audit/dismiss`, {
      key,
      type,
      reason,
    }),
  undismissAuditFinding: (deckId: number, key: string) =>
    request<{ state: DeckState; audit: AuditState }>("POST", `/api/decks/${deckId}/audit/undismiss`, { key }),
  promoteAuditFinding: (deckId: number, key: string) =>
    request<{ state: DeckState; audit: AuditState }>("POST", `/api/decks/${deckId}/audit/promote`, { key }),

  getBrief: (deckId: number) => request<{ brief: Brief }>("GET", `/api/decks/${deckId}/brief`),
  updateBrief: (deckId: number, patch: Partial<{ thesis: string; constraints_md: string }>) =>
    request<{ brief: Brief }>("PUT", `/api/decks/${deckId}/brief`, patch),
  setEngine: (deckId: number, name: string, description: string, pieces: Array<{ oracle_id: string }>) =>
    request<{ brief: Brief }>("POST", `/api/decks/${deckId}/engines`, { name, description, pieces }),
  removeEngine: (deckId: number, engineId: number) =>
    request<{ brief: Brief }>("DELETE", `/api/decks/${deckId}/engines/${engineId}`),
  ruleBriefEdit: (deckId: number, editId: number, verdict: "accept" | "reject", type?: string, reason?: string) =>
    request<{ state: DeckState; brief: Brief }>(
      "POST",
      `/api/decks/${deckId}/brief-edits/${editId}/${verdict}`,
      verdict === "reject" ? { type, reason } : undefined,
    ),
  resolveCard: (name: string) =>
    request<CardData[]>("GET", `/api/resolve?${new URLSearchParams({ name })}`),

  getChat: (deckId: number) => request<ChatMsg[]>("GET", `/api/decks/${deckId}/chat`),
  sendChat: (deckId: number, message: string) =>
    request<{ reply: string; mutated: boolean; state: DeckState }>(
      "POST",
      `/api/decks/${deckId}/chat`,
      { message },
    ),

  // Archidekt interop (spec §9)
  exportDeck: (deckId: number, opts: { categories?: boolean; onlyUnowned?: boolean } = {}) => {
    const params = new URLSearchParams();
    if (opts.categories === false) params.set("categories", "off");
    if (opts.onlyUnowned) params.set("unowned", "1");
    return request<ExportResult>("GET", `/api/decks/${deckId}/export?${params}`);
  },
  previewImport: (deckId: number, text: string) =>
    request<ImportDiff>("POST", `/api/decks/${deckId}/import/preview`, { text }),
  importList: (deckId: number, text: string, note?: string) =>
    request<{
      import: {
        applied: { added: number; cut: number; quantity_changed: number };
        log_id: number | null;
        diff: ImportDiff;
      };
      state: DeckState;
    }>("POST", `/api/decks/${deckId}/import`, { text, note }),
  addPlaytestNote: (deckId: number, note: string) =>
    request<{ state: DeckState }>("POST", `/api/decks/${deckId}/playtest-notes`, { note }),
  deletePlaytestNote: (deckId: number, noteId: number) =>
    request<{ state: DeckState }>("DELETE", `/api/decks/${deckId}/playtest-notes/${noteId}`),

  // Compaction (spec §11) and retention (spec §12)
  getContextMeter: (deckId: number) =>
    request<{ meter: ContextMeter }>("GET", `/api/decks/${deckId}/context-meter`),
  consolidate: (deckId: number) =>
    request<{ consolidation: Consolidation; state: DeckState }>(
      "POST",
      `/api/decks/${deckId}/consolidate`,
    ),
  ruleConsolidation: (deckId: number, id: number, verdict: "accept" | "reject") =>
    request<{ consolidation: Consolidation; state: DeckState; meter: ContextMeter }>(
      "POST",
      `/api/decks/${deckId}/consolidations/${id}/${verdict}`,
    ),
  getSettings: () => request<{ retention_n: number }>("GET", "/api/settings"),
  updateSettings: (patch: { retention_n?: number }) =>
    request<{ retention_n: number }>("PUT", "/api/settings", patch),

  search: (q: string, deckId?: number, filterIdentity = true) => {
    const params = new URLSearchParams({ q });
    if (deckId !== undefined) params.set("deck", String(deckId));
    if (!filterIdentity) params.set("filter", "off");
    return request<CardData[]>("GET", `/api/search?${params}`);
  },
};
