export interface CardData {
  oracle_id: string;
  name: string;
  mana_cost: string | null;
  cmc: number;
  type_line: string;
  oracle_text: string;
  power: string | null;
  toughness: string | null;
  loyalty: string | null;
  color_identity: string;
  commander_legality: string;
  is_commander: number;
}

export interface DeckCard extends CardData {
  ci_mask: number;
  slot_id: number | null;
  role: "card" | "commander" | "companion";
  owned: number;
  quantity: number;
  tag_ids: number[];
}

export interface Slot {
  id: number;
  name: string;
  target_min: number | null;
  target_max: number | null;
  position: number;
}

export interface Tag {
  id: number;
  name: string;
}

export interface SlotDelta {
  slot_id: number;
  name: string;
  count: number;
  target_min: number | null;
  target_max: number | null;
  status: "ok" | "under" | "over" | "untargeted";
  delta: number;
}

export interface Computed {
  card_count: number;
  delta_to_100: number;
  pending: {
    adds: number;
    cuts: number;
    projected_count: number;
    by_slot: Record<number, number>;
  };
  unslotted_count: number;
  slot_deltas: SlotDelta[];
  identity_violations: Array<{ oracle_id: string; name: string; color_identity: string }>;
  singleton_violations: Array<{ oracle_id: string; name: string; quantity: number; limit: number | null }>;
  legality_violations: Array<{ oracle_id: string; name: string; legality: string }>;
  land_count: number;
  curve: Record<string, number>;
  pips: Record<string, number>;
}

export interface ProposalItem {
  id: number;
  proposal_id: number;
  action: "add" | "cut";
  oracle_id: string;
  card_name: string;
  mana_cost: string | null;
  type_line: string;
  oracle_text: string;
  slot_id: number | null;
  rationale: string;
  group_id: string | null;
  status: "pending" | "accepted" | "rejected";
}

export interface Proposal {
  id: number;
  source: "manual" | "agent" | "audit" | "import";
  note: string;
  status: "open" | "resolved";
  created_at: string;
  items: ProposalItem[];
}

export interface LogEntry {
  id: number;
  revision: number;
  kind: "accept" | "reject" | "undo" | "filter_removed";
  action: "add" | "cut" | null;
  oracle_id: string | null;
  card_name: string | null;
  rationale: string | null;
  rejection_type: "hard_filter" | "thesis_change" | "playtest_finding" | "soft" | null;
  rejection_reason: string | null;
  undo_of: number | null;
  undone_by: number | null;
  brief_flag: number;
  ts: string;
}

export interface HardFilter {
  oracle_id: string;
  card_name: string;
  reason: string;
  created_at: string;
}

export interface CardNote {
  id: number;
  oracle_id: string;
  card_name: string;
  note: string;
  created_at: string;
}

export interface DraftItem {
  action: "add" | "cut";
  oracle_id: string;
  card_name: string;
  slot_id?: number | null;
  rationale: string;
  group_id?: string | null;
}

export interface AuditFinding {
  key: string;
  severity: "error" | "warn";
  title: string;
  detail: string;
  oracle_id?: string;
  card_name?: string;
  action?: "cut";
}

export interface ReasoningResult {
  summary: string;
  findings: AuditFinding[];
  dismissed: Array<AuditFinding & { dismissal: { type: string; reason: string } }>;
  dropped: number;
  error?: string;
}

/** One recorded run (spec §8) — the newest few are kept per deck. */
export interface AuditRun {
  id: number;
  deck_id: number;
  revision: number;
  instructions: string;
  status: "running" | "done" | "error";
  error: string | null;
  findings: AuditFinding[];
  reasoning: ReasoningResult | null;
  created_at: string;
  finished_at: string | null;
}

// Deterministic findings are live (recomputed per request, never stale on
// screen); the reasoning half comes from the newest run that has one.
export interface AuditState {
  revision: number;
  findings: AuditFinding[];
  dismissed: Array<
    AuditFinding & {
      dismissal: { type: string; reason: string; revision: number; created_at: string };
    }
  >;
  context: {
    card_count: number;
    delta_to_100: number;
    land_count: number;
    curve: Record<string, number>;
    pips: Record<string, number>;
    slot_deltas: SlotDelta[];
  };
  runs: AuditRun[];
  reasoning_run: AuditRun | null;
}

export interface BriefEdit {
  id: number;
  kind: "thesis" | "constraints" | "engine_set" | "engine_remove";
  payload: {
    content?: string;
    engine_name?: string;
    description?: string;
    pieces?: Array<{ oracle_id: string; note?: string }>;
  };
  rationale: string;
  source: string;
  status: string;
  created_at: string;
}

export interface Engine {
  id: number;
  name: string;
  description: string;
  pieces: Array<{ oracle_id: string; name: string; note: string; in_deck: boolean }>;
}

export interface Brief {
  thesis: string;
  constraints_md: string;
  engines: Engine[];
  updated_at: string | null;
}

export interface ChatMsg {
  id: number;
  created_at: string;
  compacted_at: string | null;
  role: "user" | "assistant" | "tool" | "system";
  content: string | null;
  tool_calls?: Array<{ id: string; function: { name: string; arguments: string } }>;
  tool_call_id?: string;
  // Set on the tool message a propose_changes call produced. `proposal` is
  // hydrated server-side on every read, so its item statuses are current
  // rather than whatever they were when the turn ran.
  proposal_id?: number;
  proposal?: Proposal | null;
}

export interface PlaytestNote {
  id: number;
  revision: number;
  note: string;
  cards: string[];
  created_at: string;
}

export interface Consolidation {
  id: number;
  status: "pending" | "accepted" | "rejected";
  summary: string;
  discarded: string[];
  rescued: Array<{ fact: string; should_have_been: string; why: string }>;
  through_message_id: number;
  message_count: number;
  brief_edit_ids: number[];
  created_at: string;
  resolved_at: string | null;
}

export interface MeterSegment {
  key: string;
  label: string;
  chars: number;
  est_tokens: number;
  share: number;
  behavior: "static" | "seldom" | "per-change" | "grows";
}

export interface ContextMeter {
  revision: number;
  retention_n: number;
  est_tokens: number;
  chars: number;
  segments: MeterSegment[];
  transcript_messages: number;
  compacted_messages: number;
  has_active_summary: boolean;
  advice: Array<{ segment: string; severity: "info" | "warn"; message: string }>;
}

export interface ExportResult {
  text: string;
  card_count: number;
  line_count: number;
  omitted_owned: number;
  revision: number;
}

export interface ImportDiff {
  adds: Array<{
    oracle_id: string;
    name: string;
    quantity: number;
    slot_id: number | null;
    slot_name: string | null;
    category: string | null;
    role: "card" | "commander" | "companion";
  }>;
  cuts: Array<{ oracle_id: string; name: string; quantity: number; role: string }>;
  quantity_changes: Array<{ oracle_id: string; name: string; from: number; to: number }>;
  unchanged: number;
  unresolved: Array<{ line_no: number; name: string; suggestions: string[] }>;
  ambiguous: Array<{ line_no: number; name: string; oracle_ids: string[] }>;
  blocked: Array<{ oracle_id: string; name: string; reason: string }>;
  unparsed: Array<{ line_no: number; raw: string }>;
  revision: number;
}

export interface DeckState {
  deck: {
    id: number;
    name: string;
    ci_mask: number | null;
    color_identity: string;
    revision: number;
  };
  slots: Slot[];
  tags: Tag[];
  cards: DeckCard[];
  computed: Computed;
  proposals: Proposal[];
  brief_edits: BriefEdit[];
  log: LogEntry[];
  hard_filters: HardFilter[];
  card_notes: CardNote[];
  playtest_notes: PlaytestNote[];
  consolidations: Consolidation[];
}

export interface DeckSummary {
  id: number;
  name: string;
  color_identity: string;
  card_count: number;
  created_at: string;
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
  createDeck: (name: string) => request<DeckState>("POST", "/api/decks", { name }),
  getDeck: (id: number) => request<DeckState>("GET", `/api/decks/${id}`),
  renameDeck: (id: number, name: string) => request<DeckState>("PATCH", `/api/decks/${id}`, { name }),
  deleteDeck: (id: number) => request<{ ok: true }>("DELETE", `/api/decks/${id}`),

  addCard: (deckId: number, oracleId: string, slotId?: number | null, role?: string) =>
    request<DeckState>("POST", `/api/decks/${deckId}/cards`, {
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
  ) => request<DeckState>("PATCH", `/api/decks/${deckId}/cards/${oracleId}`, patch),
  removeCard: (deckId: number, oracleId: string) =>
    request<DeckState>("DELETE", `/api/decks/${deckId}/cards/${oracleId}`),

  createSlot: (deckId: number, name: string, min: number | null, max: number | null) =>
    request<DeckState>("POST", `/api/decks/${deckId}/slots`, {
      name,
      target_min: min,
      target_max: max,
    }),
  updateSlot: (
    deckId: number,
    slotId: number,
    patch: Partial<{ name: string; target_min: number | null; target_max: number | null }>,
  ) => request<DeckState>("PATCH", `/api/decks/${deckId}/slots/${slotId}`, patch),
  deleteSlot: (deckId: number, slotId: number) =>
    request<DeckState>("DELETE", `/api/decks/${deckId}/slots/${slotId}`),

  createTag: (deckId: number, name: string) =>
    request<DeckState>("POST", `/api/decks/${deckId}/tags`, { name }),
  deleteTag: (deckId: number, tagId: number) =>
    request<DeckState>("DELETE", `/api/decks/${deckId}/tags/${tagId}`),

  createProposal: (deckId: number, items: DraftItem[], note?: string) =>
    request<DeckState>("POST", `/api/decks/${deckId}/proposals`, {
      items: items.map(({ card_name, ...i }) => i),
      note,
    }),
  acceptItem: (deckId: number, itemId: number) =>
    request<DeckState>("POST", `/api/decks/${deckId}/items/${itemId}/accept`),
  rejectItem: (deckId: number, itemId: number, type: string, reason: string) =>
    request<DeckState>("POST", `/api/decks/${deckId}/items/${itemId}/reject`, { type, reason }),
  undoDecision: (deckId: number, logId: number) =>
    request<DeckState>("POST", `/api/decks/${deckId}/log/${logId}/undo`),
  removeHardFilter: (deckId: number, oracleId: string) =>
    request<DeckState>("DELETE", `/api/decks/${deckId}/filters/${oracleId}`),
  addCardNote: (deckId: number, oracleId: string, note: string) =>
    request<DeckState>("POST", `/api/decks/${deckId}/notes`, { oracle_id: oracleId, note }),

  getAudit: (deckId: number) => request<AuditState>("GET", `/api/decks/${deckId}/audit`),
  // Returns as soon as the run is open — the reasoning pass finishes on the
  // server, so this resolving is not the run being done. Poll getAudit.
  startAudit: (deckId: number, instructions: string) =>
    request<AuditState & { run_id: number; already_running: boolean }>(
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

  getBrief: (deckId: number) => request<Brief>("GET", `/api/decks/${deckId}/brief`),
  updateBrief: (deckId: number, patch: Partial<{ thesis: string; constraints_md: string }>) =>
    request<Brief>("PUT", `/api/decks/${deckId}/brief`, patch),
  setEngine: (deckId: number, name: string, description: string, pieces: Array<{ oracle_id: string }>) =>
    request<Brief>("POST", `/api/decks/${deckId}/engines`, { name, description, pieces }),
  removeEngine: (deckId: number, engineId: number) =>
    request<Brief>("DELETE", `/api/decks/${deckId}/engines/${engineId}`),
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
      applied: { added: number; cut: number; quantity_changed: number };
      log_id: number | null;
      diff: ImportDiff;
      state: DeckState;
    }>(
      "POST",
      `/api/decks/${deckId}/import`,
      { text, note },
    ),
  addPlaytestNote: (deckId: number, note: string) =>
    request<DeckState>("POST", `/api/decks/${deckId}/playtest-notes`, { note }),
  deletePlaytestNote: (deckId: number, noteId: number) =>
    request<DeckState>("DELETE", `/api/decks/${deckId}/playtest-notes/${noteId}`),

  // Compaction (spec §11) and retention (spec §12)
  getContextMeter: (deckId: number) =>
    request<ContextMeter>("GET", `/api/decks/${deckId}/context-meter`),
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
