// Archidekt interop (spec §12). Deck construction happens here, playtesting
// happens in Archidekt — one-way out, one-way back. The way back applies in
// one shot: the preview diff below IS the approval step, and the import lands
// as a single undoable entry in the log. Notes from a playtest go in the
// deck's memories, stamped with the revision that was played.

import { useState } from "react";
import { api, type ImportDiff } from "./api.ts";
import { useDeck } from "./store.tsx";

export function InteropPanel() {
  const { deckId, apply } = useDeck();
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
  const [confirmingImport, setConfirmingImport] = useState(false);

  async function doExport() {
    setError(null);
    setCopied(false);
    try {
      const r = await api.exportDeck(deckId, {
        categories: withCategories,
        onlyMissing: buyList,
      });
      setExported(
        r.text ||
          "(nothing to export — the deck is empty, or every card is marked in paper and you asked for a buy list)",
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
    setConfirmingImport(false);
    setBusy(true);
    try {
      setDiff(await api.previewImport(deckId, paste));
    } catch (e: any) {
      setError(e.message);
    } finally {
      setBusy(false);
    }
  }

  // The import lands in full and at once. Destructive imports take a second,
  // in-app confirmation step beside the diff instead of leaving the app for a
  // native browser dialog.
  async function commit() {
    if (!diff) return;
    setError(null);
    setApplied(null);
    setBusy(true);
    try {
      const r = await api.importList(deckId, paste, note || undefined);
      if (r.import.log_id == null) {
        setApplied("No differences — the deck already matches that list.");
      } else {
        const { added, cut, quantity_changed } = r.import.applied;
        const bits = [`${added} added`, `${cut} cut`];
        if (quantity_changed) bits.push(`${quantity_changed} quantity changed`);
        setApplied(`Imported: ${bits.join(" · ")}. Undo it from the log if that was wrong.`);
        setPaste("");
        setNote("");
        setDiff(null);
        setConfirmingImport(false);
      }
      apply(r);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setBusy(false);
    }
  }

  const changeCount = diff
    ? diff.add_count + diff.cut_count
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
          buy list (cards missing from the paper deck)
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
          setConfirmingImport(false);
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
            <button
              className={diff.cut_count ? "danger" : ""}
              onClick={() => (diff.cut_count > 0 ? setConfirmingImport(true) : void commit())}
              disabled={busy || confirmingImport}
            >
              {diff.cut_count ? "Review & apply" : "Apply"} {changeCount} card change(s)
            </button>
          </>
        )}
      </div>
      {confirmingImport && diff && (
        <div className="action-confirm" role="alert" aria-label="Confirm destructive import">
          <strong>Remove {diff.cut_count} card(s) from this deck?</strong>
          <div className="muted rationale">
            This lands as one revision and one log entry, so the whole import can be undone.
          </div>
          <ul className="confirm-list">
            {diff.cuts.map((c) => (
              <li key={c.oracle_id}>
                {c.quantity > 1 && `${c.quantity}× `}{c.name}
                {c.role !== "card" && ` (${c.role})`}
              </li>
            ))}
            {diff.quantity_changes
              .filter((q) => q.to < q.from)
              .map((q) => (
                <li key={q.oracle_id}>{q.name} ({q.from}→{q.to})</li>
              ))}
          </ul>
          <div className="dialog-actions">
            <button autoFocus onClick={() => setConfirmingImport(false)} disabled={busy}>
              Cancel
            </button>
            <button className="destructive" onClick={() => void commit()} disabled={busy}>
              {busy ? "Applying…" : "Apply import"}
            </button>
          </div>
        </div>
      )}
      {applied && <div className="muted rationale">{applied}</div>}

      {diff && (
        <div className="import-diff">
          <div className="muted">
            {diff.add_count} to add · {diff.cut_count} to cut · {diff.unchanged} unchanged
            {diff.quantity_changes.length > 0 && ` · ${diff.quantity_changes.length} quantity`}
          </div>
          {diff.cut_count > 0 && (
            <div className="error-banner">
              Destructive: applying this removes {diff.cut_count} card(s), including quantity
              decreases. The whole import is one revision and one log entry, so undoing it puts the
              deck back exactly as it is now.
            </div>
          )}
          {diff.adds.map((a) => (
            <div key={a.oracle_id} className="log-row">
              <span className="chip ok">add</span> {a.quantity > 1 && `${a.quantity}× `}{a.name}
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
              <span className="chip over">cut</span> {c.quantity > 1 && `${c.quantity}× `}{c.name}
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
          {diff.set_aside.length > 0 && (
            <div className="muted rationale">
              Listed under Maybeboard or Sideboard, so shortlisted on the maybe list, not added:{" "}
              {diff.set_aside.map((c) => c.name).join(", ")}
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

    </>
  );
}
