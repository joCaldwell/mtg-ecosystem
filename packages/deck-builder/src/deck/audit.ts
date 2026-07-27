import type { DatabaseSync } from "node:sqlite";
import { ServiceError, getDeck } from "./service.ts";
import { createProposal, type RejectionType, REJECTION_TYPES } from "./proposals.ts";

// Deterministic checks only (spec §8.1) — SQL and code, never the model.
// The reasoning pass (§8.2) plugs in at Phase 4 and receives these results
// as context; it never re-derives them.

export interface Finding {
  key: string;
  severity: "error" | "warn";
  title: string;
  detail: string;
  oracle_id?: string;
  card_name?: string;
  // When set, the finding can be promoted into a proposal item.
  action?: "cut";
}

// Soft dismissals decay: the finding resurfaces after this many deck
// revisions (spec §7.2 — soft does not become permanent).
const SOFT_DECAY_REVISIONS = 10;

const PARTNER_MARKERS = [
  "partner", // Partner, Partner with X, partner keyword text
  "friends forever",
  "choose a background",
  "doctor's companion",
];

export function computeFindings(db: DatabaseSync, deckId: number): Finding[] {
  const { deck, cards, computed } = getDeck(db, deckId);
  const findings: Finding[] = [];

  if (computed.card_count !== 100) {
    findings.push({
      key: "card_count",
      severity: "error",
      title: `Deck is ${computed.card_count}/100`,
      detail:
        computed.delta_to_100 < 0
          ? `${-computed.delta_to_100} card(s) short of 100 (commanders count, companion doesn't).`
          : `${computed.delta_to_100} card(s) over 100.`,
    });
  }

  const commanders = cards.filter((c) => c.role === "commander");
  if (!commanders.length) {
    findings.push({
      key: "no_commander",
      severity: "error",
      title: "No commander",
      detail: "The deck has no card in the command zone.",
    });
  }
  for (const c of commanders) {
    if (!c.is_commander) {
      findings.push({
        key: `commander_ineligible:${c.oracle_id}`,
        severity: "error",
        title: `${c.name} cannot be your commander`,
        detail: "Not a legendary creature and has no “can be your commander” text.",
        oracle_id: c.oracle_id,
        card_name: c.name,
      });
    }
  }
  if (commanders.length === 2) {
    const text = (c: (typeof commanders)[0]) => c.oracle_text.toLowerCase();
    if (!commanders.every((c) => PARTNER_MARKERS.some((m) => text(c).includes(m)))) {
      findings.push({
        key: "commander_pair",
        severity: "warn",
        title: "Commander pair may be illegal",
        detail: `${commanders[0].name} + ${commanders[1].name}: both need a pairing ability (Partner, Friends forever, Choose a Background, …). Heuristic check — verify the exact pairing rule yourself.`,
      });
    }
  }

  for (const v of computed.identity_violations) {
    findings.push({
      key: `identity:${v.oracle_id}`,
      severity: "error",
      title: `${v.name} is outside the deck's color identity`,
      detail: `Card identity ${v.color_identity || "C"} does not fit ${deck.color_identity || "C"}.`,
      oracle_id: v.oracle_id,
      card_name: v.name,
      action: "cut",
    });
  }

  for (const v of computed.legality_violations) {
    findings.push({
      key: `${v.legality}:${v.oracle_id}`,
      severity: v.legality === "banned" ? "error" : "warn",
      title: `${v.name} is ${v.legality.replace("_", " ")} in Commander`,
      detail:
        v.legality === "banned"
          ? "On the Commander banned list."
          : "Scryfall marks this card as not legal in Commander.",
      oracle_id: v.oracle_id,
      card_name: v.name,
      action: "cut",
    });
  }

  for (const v of computed.singleton_violations) {
    findings.push({
      key: `singleton:${v.oracle_id}`,
      severity: "error",
      title: `${v.quantity}× ${v.name} exceeds its copy limit`,
      detail: `Limit is ${v.limit ?? "unlimited"}.`,
      oracle_id: v.oracle_id,
      card_name: v.name,
    });
  }

  for (const s of computed.slot_deltas) {
    if (s.status === "under" || s.status === "over") {
      findings.push({
        key: `slot_${s.status}:${s.slot_id}`,
        severity: "warn",
        title: `${s.name} is ${s.status} target (${s.count}/${s.target_min ?? 0}–${s.target_max ?? "∞"})`,
        detail: `${Math.abs(s.delta)} card(s) ${s.status === "under" ? "below the minimum" : "above the maximum"}. Targets are soft — dismiss if deliberate.`,
      });
    }
  }

  const companions = cards.filter((c) => c.role === "companion");
  for (const c of companions) {
    findings.push({
      key: `companion_unchecked:${c.oracle_id}`,
      severity: "warn",
      title: `${c.name}'s companion condition is not checked`,
      detail: "Deck-building conditions require the rules engine; verify by hand.",
      oracle_id: c.oracle_id,
      card_name: c.name,
    });
  }

  return findings;
}

interface DismissalRow {
  finding_key: string;
  type: RejectionType;
  reason: string;
  revision: number;
  created_at: string;
}

// Active (non-decayed) dismissals; prunes decayed soft dismissals as a side
// effect so they resurface (spec §7.2).
export function activeDismissals(db: DatabaseSync, deckId: number): Map<string, DismissalRow> {
  const { revision } = db.prepare("SELECT revision FROM decks WHERE id = ?").get(deckId) as {
    revision: number;
  };
  const dismissals = db
    .prepare("SELECT finding_key, type, reason, revision, created_at FROM audit_dismissals WHERE deck_id = ?")
    .all(deckId) as unknown as DismissalRow[];
  const active = new Map<string, DismissalRow>();
  for (const d of dismissals) {
    if (d.type === "soft" && revision - d.revision >= SOFT_DECAY_REVISIONS) {
      db.prepare("DELETE FROM audit_dismissals WHERE deck_id = ? AND finding_key = ?").run(
        deckId,
        d.finding_key,
      );
      continue;
    }
    active.set(d.finding_key, d);
  }
  return active;
}

// Current findings partitioned by dismissal state, without recording a run.
export function auditView(db: DatabaseSync, deckId: number) {
  const { deck, computed } = getDeck(db, deckId);
  const all = computeFindings(db, deckId);
  const active = activeDismissals(db, deckId);

  return {
    revision: deck.revision,
    findings: all.filter((f) => !active.has(f.key)),
    dismissed: all
      .filter((f) => active.has(f.key))
      .map((f) => ({ ...f, dismissal: active.get(f.key)! })),
    // Deterministic context the reasoning pass (§8.2) receives as given —
    // supplied here so the UI shows exactly what the model will see.
    context: {
      card_count: computed.card_count,
      delta_to_100: computed.delta_to_100,
      land_count: computed.land_count,
      curve: computed.curve,
      pips: computed.pips,
      slot_deltas: computed.slot_deltas,
    },
    // The reasoning half (§8.2) is not here: it belongs to a recorded run, not
    // to the live view. auditState() attaches the newest run that has one.
  };
}

// ---------- recorded runs (spec §8) ----------

// How many runs per deck stay on disk. Deep history has no readers: the
// deterministic half of an old run is stale the moment the deck changes, and
// what the reasoning pass said five runs ago is superseded advice.
export const AUDIT_RUN_RETENTION = 5;

export interface AuditRun {
  id: number;
  deck_id: number;
  revision: number;
  instructions: string;
  status: "running" | "done" | "error";
  error: string | null;
  /** Deterministic findings as they stood when the run started. */
  findings: Finding[];
  reasoning: ReasoningSnapshot | null;
  created_at: string;
  finished_at: string | null;
}

// Structurally the ReasoningResult from agent/reasoning.ts, redeclared here so
// the deck layer doesn't depend on the agent layer (the dependency runs the
// other way — reasoning.ts imports Finding from this file).
export interface ReasoningSnapshot {
  summary: string;
  findings: Finding[];
  dismissed: Array<Finding & { dismissal: { type: string; reason: string } }>;
  dropped: number;
  error?: string;
}

interface AuditRunRow {
  id: number;
  deck_id: number;
  revision: number;
  instructions: string;
  status: "running" | "done" | "error";
  error: string | null;
  findings_json: string;
  reasoning_json: string | null;
  created_at: string;
  finished_at: string | null;
}

function parseRun(row: AuditRunRow): AuditRun {
  return {
    id: row.id,
    deck_id: row.deck_id,
    revision: row.revision,
    instructions: row.instructions,
    status: row.status,
    error: row.error,
    findings: JSON.parse(row.findings_json) as Finding[],
    reasoning: row.reasoning_json ? (JSON.parse(row.reasoning_json) as ReasoningSnapshot) : null,
    created_at: row.created_at,
    finished_at: row.finished_at,
  };
}

// Opens a run and hands back its id. The deterministic half is complete at
// this point; the reasoning pass fills in later via finishAuditRun.
export function startAuditRun(db: DatabaseSync, deckId: number, instructions = ""): number {
  const view = auditView(db, deckId);
  const r = db
    .prepare(
      `INSERT INTO audit_runs (deck_id, revision, instructions, findings_json, status)
       VALUES (?, ?, ?, ?, 'running')`,
    )
    .run(deckId, view.revision, instructions, JSON.stringify(view.findings));
  const runId = Number(r.lastInsertRowid);
  db.prepare(
    `DELETE FROM audit_runs WHERE deck_id = ? AND id NOT IN
       (SELECT id FROM audit_runs WHERE deck_id = ? ORDER BY id DESC LIMIT ?)`,
  ).run(deckId, deckId, AUDIT_RUN_RETENTION);
  return runId;
}

export function finishAuditRun(
  db: DatabaseSync,
  runId: number,
  reasoning: object | null,
  error: string | null = null,
): void {
  db.prepare(
    `UPDATE audit_runs SET reasoning_json = ?, status = ?, error = ?, finished_at = datetime('now')
     WHERE id = ?`,
  ).run(reasoning ? JSON.stringify(reasoning) : null, error ? "error" : "done", error, runId);
}

// A run in flight dies with the process — nothing resumes it, so on boot the
// rows are marked failed rather than left claiming to be running forever.
export function reclaimStaleRuns(db: DatabaseSync): number {
  const r = db
    .prepare(
      `UPDATE audit_runs SET status = 'error', error = 'The server restarted while this run was in progress.',
         finished_at = datetime('now')
       WHERE status = 'running'`,
    )
    .run();
  return Number(r.changes);
}

export function listAuditRuns(db: DatabaseSync, deckId: number): AuditRun[] {
  return (
    db
      .prepare("SELECT * FROM audit_runs WHERE deck_id = ? ORDER BY id DESC LIMIT ?")
      .all(deckId, AUDIT_RUN_RETENTION) as unknown as AuditRunRow[]
  ).map(parseRun);
}

export function getAuditRun(db: DatabaseSync, deckId: number, runId: number): AuditRun | null {
  const row = db
    .prepare("SELECT * FROM audit_runs WHERE deck_id = ? AND id = ?")
    .get(deckId, runId) as AuditRunRow | undefined;
  return row ? parseRun(row) : null;
}

export function runningAuditRun(db: DatabaseSync, deckId: number): AuditRun | null {
  const row = db
    .prepare("SELECT * FROM audit_runs WHERE deck_id = ? AND status = 'running' ORDER BY id DESC LIMIT 1")
    .get(deckId) as AuditRunRow | undefined;
  return row ? parseRun(row) : null;
}

// What the audit section renders: deterministic findings recomputed live —
// they are SQL and are never allowed to go stale on screen — plus the recorded
// runs, of which the newest one carrying reasoning supplies the §8.2 half.
export function auditState(db: DatabaseSync, deckId: number) {
  const view = auditView(db, deckId);
  const runs = listAuditRuns(db, deckId);
  return {
    ...view,
    runs,
    reasoning_run: runs.find((r) => r.reasoning) ?? null,
  };
}

// Synchronous run — one call, recorded start to finish. The server backgrounds
// the reasoning pass instead (see startAuditRun/finishAuditRun); this stays for
// the CLI and for tests, where there is nothing to background onto.
export function runAudit(
  db: DatabaseSync,
  deckId: number,
  instructions = "",
  reasoning: object | null = null,
) {
  const runId = startAuditRun(db, deckId, instructions);
  finishAuditRun(db, runId, reasoning);
  return { run_id: runId, instructions, ...auditView(db, deckId) };
}

// Look up a finding by key across deterministic findings AND the reasoning
// findings of every retained run — so reasoning findings can be dismissed and
// promoted through the same gate, and a finding the owner is still looking at
// in the history stays actionable after a newer run lands.
function findFindingByKey(db: DatabaseSync, deckId: number, key: string): Finding | undefined {
  const deterministic = computeFindings(db, deckId).find((f) => f.key === key);
  if (deterministic) return deterministic;
  if (!key.startsWith("reasoning:")) return undefined;
  for (const run of listAuditRuns(db, deckId)) {
    const found = [...(run.reasoning?.findings ?? []), ...(run.reasoning?.dismissed ?? [])].find(
      (f) => f.key === key,
    );
    if (found) return found;
  }
  return undefined;
}

export interface FoundFinding {
  finding: Finding;
  source: "deterministic" | "reasoning";
  /** The run the finding was read out of; null when it is a live check. */
  run: AuditRun | null;
  dismissal?: { type: string; reason: string };
}

// Resolve an `audit#<run>/<key>` reference: the run named by the owner first,
// then the live deterministic checks, then any retained run. Used by the chat
// agent's get_audit tool, which must never guess at what a finding said.
export function lookupFinding(
  db: DatabaseSync,
  deckId: number,
  key: string,
  runId?: number | null,
): FoundFinding | null {
  const dismissals = activeDismissals(db, deckId);
  const dismissal = dismissals.get(key);
  const fromRun = (run: AuditRun): FoundFinding | null => {
    const deterministic = run.findings.find((f) => f.key === key);
    if (deterministic) return { finding: deterministic, source: "deterministic", run, dismissal };
    const reasoning = [...(run.reasoning?.findings ?? []), ...(run.reasoning?.dismissed ?? [])].find(
      (f) => f.key === key,
    );
    return reasoning ? { finding: reasoning, source: "reasoning", run, dismissal } : null;
  };

  if (runId != null) {
    const run = getAuditRun(db, deckId, runId);
    const hit = run && fromRun(run);
    if (hit) return hit;
  }
  const live = computeFindings(db, deckId).find((f) => f.key === key);
  if (live) return { finding: live, source: "deterministic", run: null, dismissal };
  for (const run of listAuditRuns(db, deckId)) {
    const hit = fromRun(run);
    if (hit) return hit;
  }
  return null;
}

// Dismissing a finding requires a typed reason and is logged exactly like a
// rejection (spec §8.3), with the same routing side effects.
export function dismissFinding(
  db: DatabaseSync,
  deckId: number,
  findingKey: string,
  type: RejectionType,
  reason: string,
): void {
  if (!REJECTION_TYPES.includes(type))
    throw new ServiceError(`Dismissal type must be one of: ${REJECTION_TYPES.join(", ")}`);
  if (!reason?.trim()) throw new ServiceError("Dismissing a finding requires a reason");

  const finding = findFindingByKey(db, deckId, findingKey);
  if (!finding) throw new ServiceError(`No current finding with key '${findingKey}'`, 404);

  const { revision } = db.prepare("SELECT revision FROM decks WHERE id = ?").get(deckId) as {
    revision: number;
  };

  db.exec("BEGIN");
  try {
    db.prepare(
      `INSERT INTO audit_dismissals (deck_id, finding_key, type, reason, revision)
       VALUES (?, ?, ?, ?, ?)
       ON CONFLICT(deck_id, finding_key) DO UPDATE SET type = excluded.type,
         reason = excluded.reason, revision = excluded.revision`,
    ).run(deckId, findingKey, type, reason.trim(), revision);

    db.prepare(
      `INSERT INTO decision_log
       (deck_id, revision, kind, action, oracle_id, card_name, rationale, rejection_type, rejection_reason, brief_flag)
       VALUES (?, ?, 'reject', NULL, ?, ?, ?, ?, ?, ?)`,
    ).run(
      deckId,
      revision,
      finding.oracle_id ?? null,
      finding.card_name ?? null,
      `Audit finding dismissed: ${finding.title}`,
      type,
      reason.trim(),
      type === "thesis_change" ? 1 : 0,
    );

    if (type === "playtest_finding" && finding.oracle_id) {
      db.prepare(
        "INSERT INTO card_notes (deck_id, oracle_id, card_name, note) VALUES (?, ?, ?, ?)",
      ).run(deckId, finding.oracle_id, finding.card_name ?? "", reason.trim());
    }
    db.exec("COMMIT");
  } catch (e) {
    db.exec("ROLLBACK");
    throw e;
  }
}

export function undismissFinding(db: DatabaseSync, deckId: number, findingKey: string): void {
  const r = db
    .prepare("DELETE FROM audit_dismissals WHERE deck_id = ? AND finding_key = ?")
    .run(deckId, findingKey);
  if (!r.changes) throw new ServiceError(`No dismissal for '${findingKey}'`, 404);
}

// Queue-to-promote (settled knob): a finding becomes a proposal only when
// the user promotes it. Source 'audit' so the log shows where it came from.
export function promoteFinding(db: DatabaseSync, deckId: number, findingKey: string): number {
  const finding = findFindingByKey(db, deckId, findingKey);
  if (!finding) throw new ServiceError(`No current finding with key '${findingKey}'`, 404);
  if (!finding.action || !finding.oracle_id)
    throw new ServiceError(`Finding '${finding.title}' has no directly promotable action`);
  return createProposal(
    db,
    deckId,
    [
      {
        action: finding.action,
        oracle_id: finding.oracle_id,
        rationale: `Audit: ${finding.title} — ${finding.detail}`,
      },
    ],
    { source: "audit", note: `Promoted audit finding (${finding.key})` },
  );
}

export function listDismissals(db: DatabaseSync, deckId: number) {
  return db
    .prepare("SELECT finding_key, type, reason, revision, created_at FROM audit_dismissals WHERE deck_id = ? ORDER BY created_at DESC")
    .all(deckId);
}
