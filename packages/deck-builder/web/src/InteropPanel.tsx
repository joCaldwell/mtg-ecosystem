// Archidekt interop and playtest notes (spec §9). Deck construction happens
// here, playtesting happens in Archidekt — one-way out, one-way back. The way
// back applies in one shot: the preview diff below IS the approval step, and
// the import lands as a single undoable entry in the log.

import { useState } from "react";
import { api, type DeckState, type ImportDiff } from "./api.ts";

export function InteropPanel({
  state,
  mutate,
}: {
  state: DeckState;
  mutate: (fn: () => Promise<DeckState>) => Promise<void>;
}) {
  const deckId = state.deck.id;
  const [exported, setExported] = useState<string | null>(null);
  const [withCategories, setWithCategories] = useState(true);
  const [buyList, setBuyList] = useState(false);
  const [copied, setCopied] = useState(false);

  const [paste, setPaste] = useState("");
  const [diff, setDiff] = useState<ImportDiff | null>(null);
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [applied, setApplied] = useState<string | null>(null);

  const [playtest, setPlaytest] = useState("");

  async function doExport() {
    setError(null);
    setCopied(false);
    try {
      const r = await api.exportDeck(deckId, {
        categories: withCategories,
        onlyUnowned: buyList,
      });
      setExported(
        r.text ||
          "(nothing to export — the deck is empty, or every card is marked owned and you asked for a buy list)",
      );
    } catch (e: any) {
      setError(e.message);
    }
  }

  async function copyExport() {
    if (!exported) return;
    try {
      await navigator.clipboard.writeText(exported);
      setCopied(true);
    } catch {
      setError("Clipboard blocked by the browser — select the text and copy it by hand.");
    }
  }

  async function preview() {
    setError(null);
    setBusy(true);
    try {
      setDiff(await api.previewImport(deckId, paste));
    } catch (e: any) {
      setError(e.message);
    } finally {
      setBusy(false);
    }
  }

  // The import lands in full and at once. The only thing worth stopping for is
  // the destructive half — cards in the deck that the pasted list drops.
  async function commit() {
    if (!diff) return;
    if (
      diff.cuts.length > 0 &&
      !window.confirm(
        `This import removes ${diff.cuts.length} card(s) from the deck:\n\n` +
          diff.cuts.map((c) => `· ${c.name}${c.role !== "card" ? ` (${c.role})` : ""}`).join("\n") +
          `\n\nIt lands as one entry in the log, so you can undo the whole import. Apply it?`,
      )
    )
      return;
    setError(null);
    setApplied(null);
    setBusy(true);
    try {
      const r = await api.importList(deckId, paste, note || undefined);
      if (r.log_id == null) {
        setApplied("No differences — the deck already matches that list.");
      } else {
        const bits = [`${r.applied.added} added`, `${r.applied.cut} cut`];
        if (r.applied.quantity_changed) bits.push(`${r.applied.quantity_changed} quantity changed`);
        setApplied(`Imported: ${bits.join(" · ")}. Undo it from the log if that was wrong.`);
        setPaste("");
        setNote("");
        setDiff(null);
      }
      // The composite payload is what every other mutation returns.
      await mutate(async () => r.state);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setBusy(false);
    }
  }

  const changeCount = diff
    ? diff.adds.length + diff.cuts.length + diff.quantity_changes.length
    : 0;

  // Chrome (title bar, collapse) belongs to the modal that hosts this.
  return (
    <>
      {error && <div className="error-banner">{error}</div>}

      <h3 className="interop-head">Export</h3>
      <div className="row gap wrap">
        <label className="own-toggle">
          <input
            type="checkbox"
            checked={withCategories}
            onChange={(e) => setWithCategories(e.target.checked)}
          />
          [Category] tags from slots
        </label>
        <label className="own-toggle">
          <input type="checkbox" checked={buyList} onChange={(e) => setBuyList(e.target.checked)} />
          buy list (unowned only)
        </label>
        <span className="spacer" />
        <button onClick={doExport}>Generate list</button>
        {exported && (
          <button className="small" onClick={copyExport}>
            {copied ? "copied ✓" : "copy"}
          </button>
        )}
      </div>
      {exported && (
        <>
          <textarea rows={8} readOnly value={exported} className="mono export-box" />
          <div className="muted rationale">
            Quantity + name, one card per line, with categories in brackets — Archidekt's own
            round-trip syntax. Set codes and foil markers are deliberately omitted: they break some
            importers and this app keys cards on oracle&nbsp;id, so there is nothing to lose.
          </div>
        </>
      )}

      <h3 className="interop-head">Import (paste back)</h3>
      <textarea
        rows={6}
        value={paste}
        onChange={(e) => {
          setPaste(e.target.value);
          setDiff(null);
          setApplied(null);
        }}
        placeholder={"Paste an Archidekt list here…\n1x Sol Ring [Ramp]\n1 Counterspell"}
      />
      <div className="row gap">
        <button onClick={preview} disabled={busy || !paste.trim()}>
          {busy ? "Reading…" : "Preview diff"}
        </button>
        {diff && changeCount > 0 && (
          <>
            <input
              className="grow"
              value={note}
              onChange={(e) => setNote(e.target.value)}
              placeholder="Note for the log entry (optional)"
            />
            <button className={diff.cuts.length ? "danger" : ""} onClick={commit} disabled={busy}>
              Apply {changeCount} change(s)
            </button>
          </>
        )}
      </div>
      {applied && <div className="muted rationale">{applied}</div>}

      {diff && (
        <div className="import-diff">
          <div className="muted">
            {diff.adds.length} to add · {diff.cuts.length} to cut · {diff.unchanged} unchanged
            {diff.quantity_changes.length > 0 && ` · ${diff.quantity_changes.length} quantity`}
          </div>
          {diff.cuts.length > 0 && (
            <div className="error-banner">
              Destructive: applying this removes {diff.cuts.length} card(s) that are in the deck but
              not in the pasted list. Everything else here is additive. The whole import is one log
              entry, so undoing it puts the deck back exactly as it is now.
            </div>
          )}
          {diff.adds.map((a) => (
            <div key={a.oracle_id} className="log-row">
              <span className="chip ok">add</span> {a.name}
              {a.role !== "card" && <span className="chip"> {a.role}</span>}
              {a.slot_name ? (
                <span className="muted"> → {a.slot_name}</span>
              ) : a.category ? (
                <span className="muted"> (category “{a.category}” matches no slot)</span>
              ) : null}
            </div>
          ))}
          {diff.cuts.map((c) => (
            <div key={c.oracle_id} className="log-row">
              <span className="chip over">cut</span> {c.name}
              {c.role !== "card" && <span className="muted"> ({c.role})</span>}
            </div>
          ))}
          {diff.quantity_changes.length > 0 && (
            <div className="muted rationale">
              Quantity changes applied with the rest:{" "}
              {diff.quantity_changes.map((q) => `${q.name} ${q.from}→${q.to}`).join(", ")}
            </div>
          )}
          {diff.unresolved.length > 0 && (
            <div className="error-banner">
              {diff.unresolved.length} name(s) did not match a card exactly and were skipped — no
              guessing:
              <ul>
                {diff.unresolved.map((u) => (
                  <li key={u.line_no}>
                    line {u.line_no}: “{u.name}”
                    {u.suggestions.length > 0 && ` — did you mean ${u.suggestions.join(", ")}?`}
                  </li>
                ))}
              </ul>
            </div>
          )}
          {diff.ambiguous.length > 0 && (
            <div className="muted rationale">
              {diff.ambiguous.length} name(s) matched more than one card and were skipped; add them
              through search instead.
            </div>
          )}
          {diff.blocked.length > 0 && (
            <div className="muted rationale">
              Hard-filtered, not proposed:{" "}
              {diff.blocked.map((b) => `${b.name} (“${b.reason}”)`).join(", ")}
            </div>
          )}
          {diff.unparsed.length > 0 && (
            <div className="muted rationale">
              {diff.unparsed.length} line(s) could not be read as a card entry (lines{" "}
              {diff.unparsed.map((u) => u.line_no).join(", ")}).
            </div>
          )}
        </div>
      )}

      <h3 className="interop-head">Playtest notes</h3>
      <div className="muted rationale">
        Goldfishing generates the best data in the system. A note is stamped with the deck revision
        it describes, so it stays attached to that exact list.
      </div>
      <div className="row gap">
        <input
          className="grow"
          value={playtest}
          onChange={(e) => setPlaytest(e.target.value)}
          placeholder="“Never found the engine before turn 9” — revision is stamped automatically"
        />
        <button
          disabled={!playtest.trim()}
          onClick={() =>
            mutate(() => api.addPlaytestNote(deckId, playtest)).then(() => setPlaytest(""))
          }
        >
          Add at rev {state.deck.revision}
        </button>
      </div>
      {state.playtest_notes.map((n) => (
        <div key={n.id} className="playtest-note">
          <div className="log-row">
            <span className="mono muted">rev {n.revision}</span>
            <span className="reason">{n.note}</span>
            <button
              className="icon danger"
              title="Delete note"
              onClick={() => mutate(() => api.deletePlaytestNote(deckId, n.id))}
            >
              ✕
            </button>
          </div>
          <div className="muted rationale">
            {n.cards.length} card(s) in the list this note describes · {n.created_at}
          </div>
        </div>
      ))}
      {!state.playtest_notes.length && <div className="muted rationale">No notes yet.</div>}
    </>
  );
}
