import { useCallback, useEffect, useRef, useState } from "react";
import { api, type AuditFinding, type AuditRun, type AuditState, type DeckState } from "./api.ts";
import { CardText, Markdown } from "./Markdown.tsx";

const DISMISS_TYPES = [
  { value: "soft", label: "Soft / not now (resurfaces later)" },
  { value: "thesis_change", label: "Thesis change — deck is meant to be this way" },
  { value: "playtest_finding", label: "Playtest finding — verified at the table" },
  { value: "hard_filter", label: "Hard — permanent dismissal" },
];

// While a run is in flight the server owns it; the section is just watching.
const POLL_MS = 2000;

function ago(iso: string): string {
  // SQLite writes 'YYYY-MM-DD HH:MM:SS' in UTC with no zone marker.
  const then = Date.parse(iso.replace(" ", "T") + "Z");
  if (Number.isNaN(then)) return iso;
  const secs = Math.max(0, Math.round((Date.now() - then) / 1000));
  if (secs < 60) return `${secs}s ago`;
  if (secs < 3600) return `${Math.round(secs / 60)}m ago`;
  if (secs < 86400) return `${Math.round(secs / 3600)}h ago`;
  return `${Math.round(secs / 86400)}d ago`;
}

function runLabel(run: AuditRun): string {
  const n = run.reasoning?.findings.length ?? 0;
  const state =
    run.status === "running"
      ? "running…"
      : run.status === "error"
        ? "failed"
        : run.reasoning?.error
          ? "no reasoning"
          : `${n} finding${n === 1 ? "" : "s"}`;
  return `#${run.id} · ${ago(run.created_at)} · rev ${run.revision} · ${state}`;
}

export function AuditSection({
  deckId,
  revision,
  setState,
  askAgent,
  openRef,
}: {
  deckId: number;
  /** Deck revision — deterministic findings are recomputed whenever it moves. */
  revision: number;
  setState: (s: DeckState) => void;
  /** Hand a finding to the agent chat as an audit#<run>/<key> reference. */
  askAgent: (token: string) => void;
  /** Set by the parent so the toolbar's Audit button can open and scroll here. */
  openRef?: { current: (() => void) | null };
}) {
  const [instructions, setInstructions] = useState("");
  const [audit, setAudit] = useState<AuditState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);
  const [open, setOpen] = useState(() => localStorage.getItem("deck.auditOpen") !== "0");
  const [viewRunId, setViewRunId] = useState<number | null>(null);
  const [dismissing, setDismissing] = useState<string | null>(null);
  const [dismissType, setDismissType] = useState("soft");
  const [dismissReason, setDismissReason] = useState("");
  const sectionRef = useRef<HTMLElement>(null);

  const load = useCallback(
    () => api.getAudit(deckId).then(setAudit).catch((e) => setError(e.message)),
    [deckId],
  );

  useEffect(() => {
    localStorage.setItem("deck.auditOpen", open ? "1" : "0");
  }, [open]);

  // Deterministic findings are live: every accepted proposal, cut, or import
  // moves the revision, and they are recomputed against the new list.
  useEffect(() => {
    void load();
  }, [load, revision]);

  const running = audit?.runs.some((r) => r.status === "running") ?? false;

  // A run outlives this component — it is a server-side job — so picking one
  // back up is just polling until the row stops saying 'running'.
  useEffect(() => {
    if (!running) return;
    const t = setInterval(() => void load(), POLL_MS);
    return () => clearInterval(t);
  }, [running, load]);

  useEffect(() => {
    if (!openRef) return;
    openRef.current = () => {
      setOpen(true);
      sectionRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
    };
    return () => {
      openRef.current = null;
    };
  }, [openRef]);

  async function run() {
    setStarting(true);
    setError(null);
    try {
      setAudit(await api.startAudit(deckId, instructions));
      setViewRunId(null);
      setOpen(true);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setStarting(false);
    }
  }

  async function withBoth(fn: () => Promise<{ state: DeckState; audit: AuditState }>) {
    setError(null);
    try {
      const r = await fn();
      setState(r.state);
      setAudit(r.audit);
    } catch (e: any) {
      setError(e.message);
    }
  }

  const runs = audit?.runs ?? [];
  // Default view: the newest run that actually carries a reasoning pass.
  const viewRun =
    (viewRunId != null ? runs.find((r) => r.id === viewRunId) : null) ??
    audit?.reasoning_run ??
    runs[0] ??
    null;
  const latest = runs[0] ?? null;
  const errors = audit?.findings.filter((f) => f.severity === "error").length ?? 0;
  const warns = (audit?.findings.length ?? 0) - errors;
  const reasoningCount = viewRun?.reasoning?.findings.length ?? 0;

  function FindingRow({
    f,
    dismissed,
    runId,
  }: {
    f: AuditFinding;
    dismissed?: string;
    runId: number | null;
  }) {
    // The reference the agent resolves with get_audit. Live deterministic
    // findings hang off no run, so they get the short form.
    const token = runId != null ? `audit#${runId}/${f.key}` : `audit/${f.key}`;
    return (
      <div className={`card-row finding ${f.severity}`}>
        <div className="card-main">
          <span className={`chip ${f.severity === "error" ? "over" : "under"}`}>{f.severity}</span>
          <span className="name">
            <CardText text={f.title} />
          </span>
          <span className="spacer" />
          <button
            className="small"
            title="Reference this finding in the agent chat"
            onClick={() => askAgent(token)}
          >
            ask agent
          </button>
          {!dismissed && f.action && f.oracle_id && (
            <button
              className="small"
              title="Create a proposal from this finding"
              onClick={() => withBoth(() => api.promoteAuditFinding(deckId, f.key))}
            >
              promote
            </button>
          )}
          {!dismissed ? (
            <button
              className="small danger"
              onClick={() => {
                setDismissing(dismissing === f.key ? null : f.key);
                setDismissReason("");
              }}
            >
              dismiss
            </button>
          ) : (
            <button
              className="small"
              onClick={() => withBoth(() => api.undismissAuditFinding(deckId, f.key))}
            >
              restore
            </button>
          )}
        </div>
        <div className="muted rationale">
          <Markdown text={f.detail} />
        </div>
        {dismissed && <div className="muted rationale">Dismissed: “{dismissed}”</div>}
        {dismissing === f.key && (
          <div className="row gap reject-form">
            <select value={dismissType} onChange={(e) => setDismissType(e.target.value)}>
              {DISMISS_TYPES.map((t) => (
                <option key={t.value} value={t.value}>
                  {t.label}
                </option>
              ))}
            </select>
            <input
              className="grow"
              value={dismissReason}
              onChange={(e) => setDismissReason(e.target.value)}
              placeholder="Why is this fine? (required)"
              autoFocus
            />
            <button
              className="small"
              disabled={!dismissReason.trim()}
              onClick={() =>
                withBoth(() =>
                  api.dismissAuditFinding(deckId, dismissing, dismissType, dismissReason),
                ).then(() => setDismissing(null))
              }
            >
              confirm
            </button>
          </div>
        )}
      </div>
    );
  }

  return (
    <section className="group audit-section" ref={sectionRef}>
      <h2 className="group-head audit-head">
        <button className="audit-toggle" onClick={() => setOpen(!open)} title="Collapse / expand">
          {open ? "▾" : "▸"} Audit
        </button>
        {errors > 0 && <span className="chip over">{errors} error(s)</span>}
        {warns > 0 && <span className="chip under">{warns} warning(s)</span>}
        {audit && !errors && !warns && <span className="chip ok">checks clean</span>}
        {reasoningCount > 0 && (
          <span className="chip" title="Findings from the reasoning pass">
            {reasoningCount} reasoning
          </span>
        )}
        <span className="muted audit-when">
          {running
            ? "reasoning pass running…"
            : latest
              ? `last run #${latest.id} · ${ago(latest.created_at)}`
              : "never run"}
        </span>
        <span className="rule" />
        <button className="small" onClick={run} disabled={starting || running}>
          {running ? "running…" : latest ? "Re-run" : "Run audit"}
        </button>
      </h2>

      {open && (
        <div className="audit-body">
          <div className="row gap">
            <input
              className="grow"
              value={instructions}
              onChange={(e) => setInstructions(e.target.value)}
              placeholder="Optional focus for the reasoning pass (“focus on the mana base”)"
              onKeyDown={(e) => {
                if (e.key === "Enter" && !running && !starting) void run();
              }}
            />
            {runs.length > 1 && (
              <select
                value={viewRun?.id ?? ""}
                onChange={(e) => setViewRunId(Number(e.target.value))}
                title="Recorded runs — the newest few are kept"
              >
                {runs.map((r) => (
                  <option key={r.id} value={r.id}>
                    {runLabel(r)}
                  </option>
                ))}
              </select>
            )}
          </div>

          {error && <div className="error-banner">{error}</div>}
          {running && (
            <div className="muted audit-running">
              Run #{latest?.id} is running on the server — leaving this page, or closing the
              browser, will not cancel it.
            </div>
          )}

          <h3 className="audit-sub">
            Checks <span className="muted">— live, recomputed at revision {audit?.revision}</span>
          </h3>
          {audit?.findings.map((f) => (
            <FindingRow key={f.key} f={f} runId={null} />
          ))}
          {audit && !audit.findings.length && (
            <div className="muted">✓ No deterministic findings.</div>
          )}
          {!!audit?.dismissed.length && (
            <details>
              <summary className="muted">{audit.dismissed.length} dismissed</summary>
              {audit.dismissed.map((f) => (
                <FindingRow key={f.key} f={f} dismissed={f.dismissal.reason} runId={null} />
              ))}
            </details>
          )}

          <h3 className="audit-sub">
            Reasoning pass
            {viewRun && (
              <span className="muted">
                {" "}
                — run #{viewRun.id}, {ago(viewRun.created_at)}
                {viewRun.instructions && ` · “${viewRun.instructions}”`}
              </span>
            )}
          </h3>
          {!viewRun && <div className="muted">No audit recorded yet — run one.</div>}
          {viewRun && viewRun.revision !== audit?.revision && (
            <div className="muted audit-stale">
              ⚠ Snapshot from revision {viewRun.revision}; the deck is at {audit?.revision}. The
              checks above are current, these findings may not be.
            </div>
          )}
          {viewRun?.status === "running" && <div className="muted">Waiting on the model…</div>}
          {viewRun?.status === "error" && (
            <div className="error-banner">Run failed: {viewRun.error}</div>
          )}
          {viewRun?.reasoning?.error && (
            <div className="error-banner">
              Reasoning pass unavailable: {viewRun.reasoning.error}
            </div>
          )}
          {/* A div, not a p: the summary is markdown now, and its own
              paragraphs and lists cannot legally nest inside one. */}
          {viewRun?.reasoning?.summary && (
            <div className="reasoning-summary">
              <Markdown text={viewRun.reasoning.summary} />
            </div>
          )}
          {viewRun?.reasoning?.findings.map((f) => (
            <FindingRow key={f.key} f={f} runId={viewRun.id} />
          ))}
          {viewRun?.status === "done" &&
            !viewRun.reasoning?.error &&
            !viewRun.reasoning?.findings.length && (
              <div className="muted">✓ No reasoning findings.</div>
            )}
          {!!viewRun?.reasoning?.dismissed.length && (
            <details>
              <summary className="muted">
                {viewRun.reasoning.dismissed.length} dismissed reasoning finding(s)
              </summary>
              {viewRun.reasoning.dismissed.map((f) => (
                <FindingRow key={f.key} f={f} dismissed={f.dismissal.reason} runId={viewRun.id} />
              ))}
            </details>
          )}
          {!!viewRun?.reasoning?.dropped && (
            <div className="muted">
              {viewRun.reasoning.dropped} finding(s) discarded for referencing unverifiable cards.
            </div>
          )}
        </div>
      )}
    </section>
  );
}
