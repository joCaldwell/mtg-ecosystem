import { useState } from "react";
import { api, type AuditFinding, type AuditResult, type DeckState } from "./api.ts";
import { CardText } from "./ChatPanel.tsx";

const DISMISS_TYPES = [
  { value: "soft", label: "Soft / not now (resurfaces later)" },
  { value: "thesis_change", label: "Thesis change — deck is meant to be this way" },
  { value: "playtest_finding", label: "Playtest finding — verified at the table" },
  { value: "hard_filter", label: "Hard — permanent dismissal" },
];

export function AuditPanel({
  deckId,
  setState,
}: {
  deckId: number;
  setState: (s: DeckState) => void;
}) {
  const [instructions, setInstructions] = useState("");
  const [audit, setAudit] = useState<AuditResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [dismissing, setDismissing] = useState<string | null>(null);
  const [dismissType, setDismissType] = useState("soft");
  const [dismissReason, setDismissReason] = useState("");

  async function run() {
    setBusy(true);
    setError(null);
    try {
      setAudit(await api.runAudit(deckId, instructions));
    } catch (e: any) {
      setError(e.message);
    } finally {
      setBusy(false);
    }
  }

  async function withBoth(fn: () => Promise<{ state: DeckState; audit: AuditResult }>) {
    setError(null);
    try {
      const r = await fn();
      setState(r.state);
      setAudit((prev) => ({ ...r.audit, run_id: prev?.run_id, instructions: prev?.instructions }));
    } catch (e: any) {
      setError(e.message);
    }
  }

  function FindingRow({ f, dismissed }: { f: AuditFinding; dismissed?: string }) {
    return (
      <div className={`card-row finding ${f.severity}`}>
        <div className="card-main">
          <span className={`chip ${f.severity === "error" ? "over" : "under"}`}>{f.severity}</span>
          <span className="name">
            <CardText text={f.title} />
          </span>
          <span className="spacer" />
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
          <CardText text={f.detail} />
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
    <details className="group">
      <summary>Audit</summary>
      <div className="row gap" style={{ margin: "6px 0" }}>
        <input
          className="grow"
          value={instructions}
          onChange={(e) => setInstructions(e.target.value)}
          placeholder="Optional focus for the reasoning pass (“focus on the mana base”)"
        />
        <button onClick={run} disabled={busy}>
          {busy ? "Running… (reasoning pass takes a moment)" : "Run audit"}
        </button>
      </div>
      {error && <div className="error-banner">{error}</div>}
      {audit && (
        <>
          <div className="muted">
            Run #{audit.run_id} at revision {audit.revision} — {audit.findings.length} finding(s)
            {audit.dismissed.length > 0 && `, ${audit.dismissed.length} dismissed`}
          </div>
          {audit.findings.map((f) => (
            <FindingRow key={f.key} f={f} />
          ))}
          {!audit.findings.length && <div className="muted">✓ No deterministic findings.</div>}
          {audit.dismissed.length > 0 && (
            <details>
              <summary className="muted">Dismissed findings</summary>
              {audit.dismissed.map((f) => (
                <FindingRow key={f.key} f={f} dismissed={f.dismissal.reason} />
              ))}
            </details>
          )}
          {audit.reasoning && (
            <>
              <h2>Reasoning pass</h2>
              {audit.reasoning.error && (
                <div className="error-banner">Reasoning pass unavailable: {audit.reasoning.error}</div>
              )}
              {audit.reasoning.summary && (
                <p className="reasoning-summary">
                  <CardText text={audit.reasoning.summary} />
                </p>
              )}
              {audit.reasoning.findings.map((f) => (
                <FindingRow key={f.key} f={f} />
              ))}
              {!audit.reasoning.error && !audit.reasoning.findings.length && (
                <div className="muted">✓ No reasoning findings.</div>
              )}
              {audit.reasoning.dismissed.length > 0 && (
                <details>
                  <summary className="muted">Dismissed reasoning findings</summary>
                  {audit.reasoning.dismissed.map((f) => (
                    <FindingRow key={f.key} f={f} dismissed={f.dismissal.reason} />
                  ))}
                </details>
              )}
              {audit.reasoning.dropped > 0 && (
                <div className="muted">
                  {audit.reasoning.dropped} finding(s) discarded for referencing unverifiable cards.
                </div>
              )}
            </>
          )}
        </>
      )}
    </details>
  );
}
