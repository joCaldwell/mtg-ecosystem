// Segmented context meter, the consolidate command, and the retention-N
// control (spec §11–§12). These live together on purpose: the meter's whole
// job is telling you which of the two levers to pull — compact the
// transcript, or retune retention.

import { useEffect, useState } from "react";
import { api } from "./api.ts";
import { useDeck } from "./store.tsx";
import { Markdown } from "./Markdown.tsx";

const BEHAVIOR_HINT: Record<string, string> = {
  static: "static across all decks",
  seldom: "seldom changes — cached prefix",
  "per-change": "changes when the deck changes",
  grows: "grows over time",
};

export function SessionPanel() {
  const { deckId, state, meter, apply } = useDeck();
  const [retention, setRetention] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function load() {
    setError(null);
    try {
      const [m, s] = await Promise.all([api.getContextMeter(deckId), api.getSettings()]);
      apply(m);
      setRetention(s.retention_n);
    } catch (e: any) {
      setError(e.message);
    }
  }

  // Measuring the context costs a request, so it used to wait for the panel to
  // be expanded. Now that the panel only exists while its modal is open,
  // mounting IS the "opened it" signal.
  useEffect(() => {
    setRetention(null);
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [deckId]);

  async function saveRetention(n: number) {
    setError(null);
    try {
      const s = await api.updateSettings({ retention_n: n });
      setRetention(s.retention_n);
      apply(await api.getContextMeter(deckId));
    } catch (e: any) {
      setError(e.message);
    }
  }

  async function consolidate() {
    setBusy(true);
    setError(null);
    try {
      apply(await api.consolidate(deckId));
    } catch (e: any) {
      setError(e.message);
    } finally {
      setBusy(false);
    }
  }

  async function rule(id: number, verdict: "accept" | "reject") {
    setError(null);
    try {
      apply(await api.ruleConsolidation(deckId, id, verdict));
    } catch (e: any) {
      setError(e.message);
    }
  }

  const pending = state?.consolidations ?? [];
  const max = meter ? Math.max(...meter.segments.map((s) => s.est_tokens), 1) : 1;

  // Chrome (title bar, collapse) belongs to the modal that hosts this.
  return (
    <>
      {error && <div className="error-banner">{error}</div>}

      <div className="row gap wrap">
        <button className="small" onClick={load}>
          {meter ? "refresh meter" : "measure context"}
        </button>
        {meter && (
          <>
            <span className="chip">~{Math.round(meter.est_tokens / 100) / 10}k est. tokens</span>
            <span className="chip">{meter.transcript_messages} messages resident</span>
            {meter.compacted_messages > 0 && (
              <span className="chip" title="Still on disk — compaction is non-destructive">
                {meter.compacted_messages} compacted
              </span>
            )}
          </>
        )}
        <span className="spacer" />
        <button onClick={consolidate} disabled={busy || pending.length > 0}>
          {busy ? "Consolidating…" : "Consolidate"}
        </button>
      </div>

      {meter && (
        <>
          <table className="meter">
            <tbody>
              {meter.segments.map((s) => (
                <tr key={s.key}>
                  <td className="meter-label">{s.label}</td>
                  <td className="meter-bar-cell">
                    <div
                      className={`meter-bar ${s.behavior === "grows" ? "grows" : ""}`}
                      style={{ width: `${(s.est_tokens / max) * 100}%` }}
                    />
                  </td>
                  <td className="mono meter-num">
                    {s.est_tokens >= 1000
                      ? `${Math.round(s.est_tokens / 100) / 10}k`
                      : s.est_tokens}
                  </td>
                  <td className="muted meter-hint">{BEHAVIOR_HINT[s.behavior]}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {meter.advice.map((a, i) => (
            <div key={i} className={a.severity === "warn" ? "error-banner" : "muted rationale"}>
              {a.message}
            </div>
          ))}
          <div className="muted rationale">
            Token counts are estimated at ~4 characters each — precise enough to tell you which
            segment is the problem, which is the only question this meter answers.
          </div>

          <div className="row gap retention-row">
            <label className="muted">Decision-log retention N</label>
            <input
              type="number"
              min={1}
              max={500}
              value={retention ?? ""}
              style={{ width: "5em" }}
              onChange={(e) => setRetention(Number(e.target.value))}
              onBlur={(e) => saveRetention(Number(e.target.value))}
            />
            <span className="muted rationale">
              last N decisions stay in context. Hard filters and playtest findings are kept forever
              regardless.
            </span>
          </div>
        </>
      )}

      {pending.map((c) => (
        <div key={c.id} className="proposal">
          <div className="row gap">
            <b>Consolidation #{c.id}</b>
            <span className="muted">{c.message_count} messages would leave context</span>
            <span className="spacer" />
            <button className="small" onClick={() => rule(c.id, "accept")}>
              accept
            </button>
            <button className="small danger" onClick={() => rule(c.id, "reject")}>
              reject
            </button>
          </div>
          <div className="chat-msg assistant" style={{ maxWidth: "100%" }}>
            <Markdown text={c.summary} />
          </div>
          {c.discarded.length > 0 && (
            <details>
              <summary className="muted">Proposed discards ({c.discarded.length})</summary>
              {c.discarded.map((d, i) => (
                <div key={i} className="log-row">
                  {d}
                </div>
              ))}
            </details>
          )}
          {c.rescued.length > 0 && (
            <div className="rescued">
              <b>Rescued from the transcript ({c.rescued.length})</b>
              <div className="muted rationale">
                Facts that were only living in the chat. If the same kind keeps showing up here, a
                tool is missing — that fact should have been written to a record when it happened.
              </div>
              {c.rescued.map((r, i) => (
                <div key={i} className="log-row">
                  {r.fact}
                  <span className="muted"> → belongs in: {r.should_have_been}</span>
                  {r.why && <div className="muted rationale">{r.why}</div>}
                </div>
              ))}
            </div>
          )}
          {c.brief_edit_ids.length > 0 && (
            <div className="muted rationale">
              It also proposed {c.brief_edit_ids.length} brief edit(s) — rule on those separately in
              the proposals section; they are independent of this consolidation.
            </div>
          )}
          <div className="muted rationale">
            Accepting moves those messages out of context. They stay on disk, and the deck, brief,
            hard filters, playtest notes, and pending proposals are untouched either way.
          </div>
        </div>
      ))}
    </>
  );
}
