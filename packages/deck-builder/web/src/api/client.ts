// Typed client. Wire types are imported from the server source (type-only,
// so they are erased at build and Vite never bundles server code) — a field
// change on the server fails this typecheck instead of failing at runtime.

import type { DeckState } from "../../../src/state.ts";
import type { ActivityEntry } from "../../../src/activity.ts";
import type { CardRow } from "../../../src/cards.ts";
import type { EngineView } from "../../../src/deck/brief.ts";
import type { Finding } from "../../../src/deck/checks.ts";
import type { DeckComparison } from "../../../src/deck/fork.ts";
import type { ExportResult, ImportDiff } from "../../../src/deck/interop.ts";
import type { CardRuling, LogEntry } from "../../../src/deck/log.ts";
import type { PaperStatus } from "../../../src/deck/paper.ts";
import type { ProposalView } from "../../../src/deck/proposals.ts";
import type { DeckSummary } from "../../../src/deck/service.ts";
import type { DocumentEditView, ProfileView } from "../../../src/documents.ts";
import type { MemoryView, Revision } from "../../../src/memory.ts";
import type { SearchResult } from "../../../src/search/index.ts";
import type { PlayerStats } from "../../../src/stats.ts";

export type {
  ActivityEntry,
  CardRuling,
  DeckComparison,
  DeckState,
  DeckSummary,
  ExportResult,
  Finding,
  ImportDiff,
  LogEntry,
  PaperStatus,
  PlayerStats,
};
export type CardData = SearchResult;
export type DeckCard = DeckState["cards"][number];
export type Slot = DeckState["slots"][number];
export type Tag = DeckState["tags"][number];
export type Computed = DeckState["computed"];
export type SlotDelta = Computed["slot_deltas"][number];
export type Proposal = ProposalView;
export type ProposalItem = Proposal["items"][number];
export type DocumentEdit = DocumentEditView;
export type Brief = DeckState["brief"];
export type Engine = EngineView;
export type Memory = MemoryView;
export type MemoryRevision = Revision;
export type Profile = ProfileView;

export interface Inbox {
  proposals: Proposal[];
  document_edits: DocumentEdit[];
  /** Rulings no agent has read yet. */
  unreviewed: number;
}

export interface Settings {
  budgets: { global: number; deck: number };
  cards: { count: number; updated_at: string | null };
  agent: { url: string; token: string; stdio_command: string };
}

export interface MemoryList {
  memories: Memory[];
  budgets: { global: number; deck: number };
  global_budget: { budget: number; used: number; omitted: number };
}

// The server's envelope rule: a response carrying a domain composite names
// it. The deck store's apply() takes `state`; pages take the rest.
export interface Envelope {
  state?: DeckState;
  inbox?: Inbox;
  profile?: Profile;
}

export class ApiError extends Error {}

async function request<T>(method: string, path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  const res = await fetch(path, {
    method,
    headers: body !== undefined ? { "Content-Type": "application/json" } : undefined,
    body: body !== undefined ? JSON.stringify(body) : undefined,
    signal,
  });
  const data = await res.json();
  if (!res.ok) throw new ApiError(data.error ?? `HTTP ${res.status}`);
  return data as T;
}

type StateResponse = { state: DeckState };
export type Outcome = "accept" | "reject";

export const api = {
  // ---------- decks ----------
  listDecks: () => request<DeckSummary[]>("GET", "/api/decks"),
  createDeck: (name: string) => request<StateResponse>("POST", "/api/decks", { name }),
  getDeck: (id: number) => request<StateResponse>("GET", `/api/decks/${id}`),
  renameDeck: (id: number, name: string) => request<StateResponse>("PATCH", `/api/decks/${id}`, { name }),
  setDeckStatus: (id: number, status: "draft" | "active") =>
    request<StateResponse>("PATCH", `/api/decks/${id}`, { status }),
  setDeckParent: (id: number, parentId: number | null) =>
    request<StateResponse>("PATCH", `/api/decks/${id}`, { parent_id: parentId }),
  deleteDeck: (id: number) => request<{ ok: true }>("DELETE", `/api/decks/${id}`),
  forkDeck: (id: number, name: string) => request<StateResponse>("POST", `/api/decks/${id}/fork`, { name }),
  compareDecks: (id: number, other?: number) =>
    request<DeckComparison>("GET", `/api/decks/${id}/compare${other ? `?other=${other}` : ""}`),
  paperStatus: (id: number) => request<PaperStatus>("GET", `/api/decks/${id}/paper`),

  // ---------- cards in a deck ----------
  addCard: (deckId: number, oracleId: string, opts: { slotId?: number | null; role?: string; maybe?: boolean } = {}) =>
    request<StateResponse>("POST", `/api/decks/${deckId}/cards`, {
      oracle_id: oracleId,
      slot_id: opts.slotId ?? null,
      role: opts.role,
      maybe: opts.maybe,
    }),
  updateCard: (
    deckId: number,
    oracleId: string,
    patch: Partial<{
      slot_id: number | null;
      role: string;
      in_paper: boolean;
      quantity: number;
      tag_ids: number[];
      /** true parks the card on the maybe list; false promotes it into the deck. */
      maybe: boolean;
    }>,
  ) => request<StateResponse>("PATCH", `/api/decks/${deckId}/cards/${oracleId}`, patch),
  removeCard: (deckId: number, oracleId: string) =>
    request<StateResponse>("DELETE", `/api/decks/${deckId}/cards/${oracleId}`),
  setAllInPaper: (deckId: number, inPaper: boolean) =>
    request<StateResponse>("PATCH", `/api/decks/${deckId}/cards`, { in_paper: inPaper }),
  cardHistory: (deckId: number, oracleId: string) =>
    request<{ rulings: CardRuling[]; memories: Memory[] }>(
      "GET",
      `/api/decks/${deckId}/cards/${oracleId}/history`,
    ),

  // ---------- slots and tags ----------
  createSlot: (deckId: number, name: string, min: number | null, max: number | null) =>
    request<StateResponse>("POST", `/api/decks/${deckId}/slots`, { name, target_min: min, target_max: max }),
  updateSlot: (
    deckId: number,
    slotId: number,
    patch: Partial<{ name: string; target_min: number | null; target_max: number | null }>,
  ) => request<StateResponse>("PATCH", `/api/decks/${deckId}/slots/${slotId}`, patch),
  deleteSlot: (deckId: number, slotId: number) =>
    request<StateResponse>("DELETE", `/api/decks/${deckId}/slots/${slotId}`),
  createTag: (deckId: number, name: string) =>
    request<StateResponse>("POST", `/api/decks/${deckId}/tags`, { name }),
  renameTag: (deckId: number, tagId: number, name: string) =>
    request<StateResponse>("PATCH", `/api/decks/${deckId}/tags/${tagId}`, { name }),
  deleteTag: (deckId: number, tagId: number) =>
    request<StateResponse>("DELETE", `/api/decks/${deckId}/tags/${tagId}`),

  // ---------- rulings (spec §8) ----------
  getInbox: () => request<Inbox>("GET", "/api/inbox"),
  ruleItems: (deckId: number, itemIds: number[], outcome: Outcome, reason = "") =>
    request<StateResponse & { inbox: Inbox }>("POST", `/api/decks/${deckId}/items/rule`, {
      item_ids: itemIds,
      outcome,
      reason,
    }),
  ruleProposal: (deckId: number, proposalId: number, outcome: Outcome, reason = "") =>
    request<StateResponse & { inbox: Inbox }>("POST", `/api/decks/${deckId}/proposals/${proposalId}/rule`, {
      outcome,
      reason,
    }),
  ruleDocumentEdit: (editId: number, outcome: Outcome, reason = "") =>
    request<Envelope & { inbox: Inbox }>("POST", `/api/document-edits/${editId}/rule`, { outcome, reason }),
  undo: (deckId: number, logId: number) =>
    request<StateResponse & { skipped: string[] }>("POST", `/api/decks/${deckId}/log/${logId}/undo`),

  // ---------- documents (spec §6) ----------
  updateBrief: (deckId: number, patch: Partial<{ thesis: string; constraints_md: string }>) =>
    request<StateResponse>("PUT", `/api/decks/${deckId}/brief`, patch),
  setEngine: (deckId: number, name: string, description: string, pieces: Array<{ oracle_id: string }>) =>
    request<StateResponse>("POST", `/api/decks/${deckId}/engines`, { name, description, pieces }),
  removeEngine: (deckId: number, engineId: number) =>
    request<StateResponse>("DELETE", `/api/decks/${deckId}/engines/${engineId}`),
  getProfile: () => request<{ profile: Profile }>("GET", "/api/profile"),
  setProfile: (bodyMd: string) => request<{ profile: Profile }>("PUT", "/api/profile", { body_md: bodyMd }),

  // ---------- memories (spec §7.5) ----------
  listMemories: (q: { scope?: string; deck?: number; card?: string; q?: string; status?: string } = {}) => {
    const params = new URLSearchParams();
    for (const [k, v] of Object.entries(q)) if (v !== undefined && v !== "") params.set(k, String(v));
    return request<MemoryList>("GET", `/api/memories?${params}`);
  },
  addMemory: (input: { text: string; scope: "global" | "deck"; deck_id?: number | null }) =>
    request<{ memory: Memory }>("POST", "/api/memories", input),
  reviseMemory: (id: number, revision: MemoryRevision) =>
    request<{ memory: Memory }>("PATCH", `/api/memories/${id}`, revision),

  // ---------- activity, stats, focus, settings ----------
  listActivity: () => request<ActivityEntry[]>("GET", "/api/activity"),
  getStats: () => request<PlayerStats>("GET", "/api/stats"),
  setFocus: (deckId: number | null, selected: string[] = []) =>
    request<{ ok: true }>("PUT", "/api/focus", { deck_id: deckId, selected }),
  getSettings: () => request<Settings>("GET", "/api/settings"),
  updateSettings: (patch: { budgets?: { global?: number; deck?: number } }) =>
    request<Settings>("PUT", "/api/settings", patch),
  rotateToken: () => request<Settings>("POST", "/api/settings/rotate-token"),
  refreshCards: () => request<Settings>("POST", "/api/cards/refresh"),

  // ---------- Archidekt interop (spec §12) ----------
  exportDeck: (deckId: number, opts: { categories?: boolean; onlyMissing?: boolean } = {}) => {
    const params = new URLSearchParams();
    if (opts.categories === false) params.set("categories", "off");
    if (opts.onlyMissing) params.set("missing", "1");
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

  // ---------- card search ----------
  resolveCard: (name: string) =>
    request<{ matches: CardRow[]; suggestions: string[] }>("GET", `/api/resolve?${new URLSearchParams({ name })}`),
  search: (q: string, deckId?: number, filterIdentity = true, signal?: AbortSignal) => {
    const params = new URLSearchParams({ q });
    if (deckId !== undefined) params.set("deck", String(deckId));
    if (!filterIdentity) params.set("filter", "off");
    return request<CardData[]>("GET", `/api/search?${params}`, undefined, signal);
  },
};
