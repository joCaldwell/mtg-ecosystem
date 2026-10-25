import type { accountInfo, adminInfo } from "../../../src/accounts.ts";
import type { AgentToken } from "../../../src/tokens.ts";
import type { PlaytestView, PlaytestAction, PlaytestUiPatch, TableCard, TableObject, CardFace, Zone, Row as TableRow, Mode } from "../../../src/playtest/service.ts";
export type { PlaytestView, PlaytestAction, PlaytestUiPatch, TableCard, TableObject, CardFace, Zone, TableRow, Mode };
// Typed client. Wire types are imported from the server source (type-only,
// so they are erased at build and Vite never bundles server code) — a field
// change on the server fails this typecheck instead of failing at runtime.

import type { PowerProfile, PowerTargetInput, PowerAssessmentInput, BracketMapping } from "../../../src/deck/power.ts";
import type { DeckState } from "../../../src/state.ts";
import type { ActivityEntry } from "../../../src/activity.ts";
import type { CardRow } from "../../../src/cards.ts";
import type { EngineView } from "../../../src/deck/brief.ts";
import type { Finding } from "../../../src/deck/checks.ts";
import type { DeckComparison } from "../../../src/deck/fork.ts";
import type { ExportOptions, ExportResult, ImportDiff } from "../../../src/deck/interop.ts";
import type { CardRuling, LogEntry } from "../../../src/deck/log.ts";
import type { ProposalView } from "../../../src/deck/proposals.ts";
import type { DeckCategory, DeckSummary } from "../../../src/deck/service.ts";
import type { DocumentEditView, ProfileView } from "../../../src/documents.ts";
import type { MemoryView, Revision } from "../../../src/memory.ts";
import type { Jobs } from "../../../src/jobs.ts";
import type { PriceStatus } from "../../../src/prices.ts";
import type { SearchResult } from "../../../src/search/index.ts";
import type { PlayerStats } from "../../../src/stats.ts";

export type {
  PowerProfile,
  PowerTargetInput,
  PowerAssessmentInput,
  BracketMapping,
  ActivityEntry,
  CardRuling,
  DeckComparison,
  DeckCategory,
  DeckState,
  DeckSummary,
  ExportResult,
  Finding,
  ImportDiff,
  LogEntry,
  PlayerStats,
};
export type Power = DeckState["power"];
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

export interface Session { mode: "local" | "hosted"; providers: string[]; user: ReturnType<typeof accountInfo> | null }

export interface Settings {
  jobs: ReturnType<Jobs["status"]> | null;
  budgets: { global: number; deck: number };
  cards: { count: number; updated_at: string | null; prices: PriceStatus };
  agent: { url: string; mode: "local" | "hosted"; tokens: AgentToken[]; oauth_usage?: { client_id:string; label:string; last_used_at:string }[]; stdio_command?: string };
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

export class ApiError extends Error {
  status: number;
  error: string;
  constructor(status: number, error: string) { super(error); this.status = status; this.error = error; }
}

async function request<T>(method: string, path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  const res = await fetch(path, {
    method,
    credentials: "same-origin",
    headers: body !== undefined ? { "Content-Type": "application/json" } : undefined,
    body: body !== undefined ? JSON.stringify(body) : undefined,
    signal,
  });
  const json = res.headers.get("content-type")?.split(";")[0].trim() === "application/json";
  if (!res.ok) {
    const data = json ? await res.json().catch(() => null) : null;
    if (typeof window !== "undefined") window.dispatchEvent(new CustomEvent("api-error", { detail: { status: res.status, retryAfter: Number(res.headers.get("Retry-After") ?? 60), message: data?.error } }));
    throw new ApiError(res.status, typeof data?.error === "string" ? data.error : `HTTP ${res.status}`);
  }
  if (!json) throw new ApiError(res.status, "The server returned an unexpected response. Please retry.");
  const data = await res.json().catch(() => { throw new ApiError(res.status, "The server returned invalid JSON. Please retry."); });
  return data as T;
}

type StateResponse = { state: DeckState };
export type Outcome = "accept" | "reject";

export const api = {
  session: () => request<Session>("GET","/api/session"),
  signup: (code:string) => request("POST","/api/signup",{code}),
  signIn: (provider:string, oauth_query:string) => request<{url:string}>("POST","/api/auth/sign-in/social",{provider,callbackURL:location.origin + "/",errorCallbackURL:location.origin + "/signin",...(oauth_query && new URLSearchParams(oauth_query).has("sig") ? {oauth_query} : {})}),
  signOut: () => request("POST","/api/auth/sign-out",{}),
  chooseHandle: (handle:string) => request("PATCH","/api/account/handle",{handle}),
  exportAccount: () => request<unknown>("GET","/api/account/export"),
  deleteAccount: (handle:string) => request("DELETE","/api/account",{handle}),
  admin: () => request<ReturnType<typeof adminInfo> & { jobs:Settings["jobs"] }>("GET","/api/admin"),
  createInvite: () => request<{code:string}>("POST","/api/admin/invites",{}),
  enqueueJob: (job:"refresh"|"backup") => request("POST","/api/admin/jobs",{job}),
  tableCard: (oracleId: string) => request<{ card: TableCard }>("GET", `/api/cards/${oracleId}/table`),
  getPlaytest: (deckId: number) => request<{ playtest: PlaytestView | null }>("GET", `/api/decks/${deckId}/playtest`),
  startPlaytest: (deckId: number, mode: Mode, previous?: PlaytestView, replace = false) => request<{ playtest: PlaytestView }>("POST", `/api/decks/${deckId}/playtest`, { mode, replace, session_id: previous?.session.id, expected_version: previous?.session.version }),
  playtestAction: (deckId: number, sessionId: string, version: number, action: PlaytestAction) => request<{ playtest: PlaytestView }>("POST", `/api/decks/${deckId}/playtest/actions`, { session_id: sessionId, expected_version: version, action }),
  playtestUi: (deckId: number, sessionId: string, version: number, ui: PlaytestUiPatch) => request<{ playtest: PlaytestView }>("PATCH", `/api/decks/${deckId}/playtest/ui`, { session_id: sessionId, expected_version: version, ui }),
  playtestObservation: (deckId: number, input: { session_id: string; request_id: string; text: string; oracle_id: string | null; deck_revision?: number }) => request<{ memory: Memory }>("POST", `/api/decks/${deckId}/playtest/observations`, input),

  // ---------- decks ----------
  listDecks: () => request<DeckSummary[]>("GET", "/api/decks"),
  createDeck: (name: string, category: DeckCategory = "theorycrafted") =>
    request<StateResponse>("POST", "/api/decks", { name, category }),
  getDeck: (id: number) => request<StateResponse>("GET", `/api/decks/${id}`),
  renameDeck: (id: number, name: string) => request<StateResponse>("PATCH", `/api/decks/${id}`, { name }),
  setDeckCategory: (id: number, category: DeckCategory) =>
    request<StateResponse>("PATCH", `/api/decks/${id}`, { category }),
  setDeckParent: (id: number, parentId: number | null) =>
    request<StateResponse>("PATCH", `/api/decks/${id}`, { parent_id: parentId }),
  deleteDeck: (id: number) => request<{ ok: true }>("DELETE", `/api/decks/${id}`),
  forkDeck: (id: number, name: string) => request<StateResponse>("POST", `/api/decks/${id}/fork`, { name }),
  compareDecks: (id: number, other?: number) =>
    request<DeckComparison>("GET", `/api/decks/${id}/compare${other ? `?other=${other}` : ""}`),

  acceptProposals: (deckId: number, proposals: Array<{ proposal_id: number; acknowledge_stale?: boolean }>, expected_revision: number) =>
    request<{ state: DeckState; inbox: Inbox }>("POST", `/api/decks/${deckId}/proposals/rule`, { proposals, expected_revision }),

  // ---------- cards in a deck ----------
  addCard: (deckId: number, oracleId: string, opts: { slotId?: number | null; secondarySlotIds?: number[]; role?: string; maybe?: boolean } = {}) =>
    request<StateResponse>("POST", `/api/decks/${deckId}/cards`, {
      oracle_id: oracleId,
      slot_id: opts.slotId ?? null,
      secondary_slot_ids: opts.secondarySlotIds,
      role: opts.role,
      maybe: opts.maybe,
    }),
  updateCard: (
    deckId: number,
    oracleId: string,
    patch: Partial<{
      slot_id: number | null;
      secondary_slot_ids: number[];
      role: string;
      quantity: number;
      tag_ids: number[];
      /** true parks the card on the maybe list; false promotes it into the deck. */
      maybe: boolean;
    }>,
  ) => request<StateResponse>("PATCH", `/api/decks/${deckId}/cards/${oracleId}`, patch),
  removeCard: (deckId: number, oracleId: string) =>
    request<StateResponse>("DELETE", `/api/decks/${deckId}/cards/${oracleId}`),
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
  proposalHistory: () => request<{ proposals: Proposal[]; document_edits: DocumentEdit[] }>("GET", "/api/proposal-history"),
  ruleItems: (deckId: number, itemIds: number[], outcome: Outcome, reason = "", options: { expected_revision?: number; acknowledge_stale?: boolean } = {}) =>
    request<StateResponse & { inbox: Inbox }>("POST", `/api/decks/${deckId}/items/rule`, {
      ...options,
      item_ids: itemIds,
      outcome,
      reason,
    }),
  ruleProposal: (deckId: number, proposalId: number, outcome: Outcome, reason = "", options: { expected_revision?: number; acknowledge_stale?: boolean } = {}) =>
    request<StateResponse & { inbox: Inbox }>("POST", `/api/decks/${deckId}/proposals/${proposalId}/rule`, {
      ...options,
      outcome,
      reason,
    }),
  ruleDocumentEdit: (editId: number, outcome: Outcome, reason = "", options: { expected_revision?: number; acknowledge_stale?: boolean; expected_documents?: Record<string, string> } = {}) =>
    request<Envelope & { inbox: Inbox }>("POST", `/api/document-edits/${editId}/rule`, { outcome, reason, ...options }),
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

  // ---------- deck power (spec §5.7) ----------
  setPowerTarget: (deckId: number, target: PowerTargetInput | null) =>
    request<{ state: DeckState }>("PUT", `/api/decks/${deckId}/power/target`, { target }),
  assessPower: (deckId: number, assessment: PowerAssessmentInput) =>
    request<{ state: DeckState; assessment_id: number }>("POST", `/api/decks/${deckId}/power/assessments`, assessment),

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
  createToken: (label: string) => request<{ token: string; credential: AgentToken }>("POST", "/api/settings/tokens", { label }),
  revokeToken: (id: number) => request<{ ok: true }>("DELETE", `/api/settings/tokens/${id}`),
  refreshCards: () => request<Settings>("POST", "/api/cards/refresh"),

  // ---------- Archidekt interop (spec §12) ----------
  exportDeck: (deckId: number, opts: ExportOptions = {}) => {
    const params = new URLSearchParams();
    if (opts.categories === false) params.set("categories", "off");
    if (opts.oracleIds !== undefined) params.set("oracle_ids", opts.oracleIds.join(","));
    if (opts.includeMaybe) params.set("include_maybe", "on");
    return request<ExportResult>("GET", `/api/decks/${deckId}/export?${params}`);
  },
  previewImport: (deckId: number, text: string) =>
    request<ImportDiff>("POST", `/api/decks/${deckId}/import/preview`, { text }),
  importList: (deckId: number, text: string, note?: string) =>
    request<{
      import: {
        applied: { added: number; cut: number; quantity_changed: number; role_changed: number };
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
