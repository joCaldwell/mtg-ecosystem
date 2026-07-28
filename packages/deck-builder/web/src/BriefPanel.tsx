import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import { api, type Engine } from "./api.ts";
import { useDeck } from "./store.tsx";
import { usePeekProps } from "./CardPeek.tsx";
import { useAutosize } from "./lib.ts";
import { CardText, Markdown } from "./Markdown.tsx";

type EngineForm = { name: string; description: string; pieces: string; existing: boolean };

/**
 * A brief field at rest is prose, not a form.
 *
 * The thesis is read far more often than it is rewritten — you open this panel
 * to check what the deck said it was doing — and a permanently-live textarea
 * served that badly: [[Card Name]] showed as literal brackets, the markdown the
 * agent writes into a brief showed as literal asterisks, and a paragraph got
 * clipped into a three-row scroller. At rest it now goes through the same
 * Markdown path as every other agent output, card refs peekable and all; it
 * becomes an editor only when asked.
 */
function Field({
  label,
  value,
  placeholder,
  onSave,
}: {
  label: string;
  value: string;
  placeholder: string;
  /** Rejects on failure, in which case the draft stays open and unlost. */
  onSave: (next: string) => Promise<void>;
}) {
  // The draft doubles as the mode: null is reading, a string is editing. One
  // piece of state means a stale draft can't outlive the editor it belongs to.
  const [draft, setDraft] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const ref = useRef<HTMLTextAreaElement>(null);

  // Height follows the wrapped text, not the line count — a thesis is one long
  // paragraph with no newlines in it, and counting them would open a four-row
  // scroller over an eight-row field. Capped by max-height in the stylesheet;
  // the +2 puts the textarea's borders back.
  useAutosize(ref, draft, 2);

  useEffect(() => {
    if (draft === null) return;
    const el = ref.current;
    if (!el) return;
    el.focus();
    // Caret at the end: the usual edit appends a clause rather than replacing
    // the field, and a select-all would be one keystroke from losing the text.
    el.setSelectionRange(el.value.length, el.value.length);
    // Only when the editor opens — re-running on every keystroke would fight
    // the caret.
  }, [draft === null]);

  async function commit() {
    if (draft === null || busy) return;
    setBusy(true);
    try {
      await onSave(draft);
      setDraft(null);
    } catch {
      // The panel above renders the message; staying open keeps the text.
    } finally {
      setBusy(false);
    }
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      // Stops here rather than reaching the modal's document-level listener,
      // which would close the whole panel and take the edit with it.
      e.stopPropagation();
      setDraft(null);
    }
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) commit();
  }

  return (
    <section className="brief-field">
      <div className="field-head">
        <h2>{label}</h2>
        <span className="spacer" />
        {draft === null ? (
          <button className="small" onClick={() => setDraft(value)}>
            Edit
          </button>
        ) : (
          <>
            <button className="small" onClick={() => setDraft(null)} disabled={busy}>
              Cancel
            </button>
            <button className="small primary" onClick={commit} disabled={busy || draft === value}>
              {busy ? "Saving…" : "Save"}
            </button>
          </>
        )}
      </div>
      {draft === null ? (
        value.trim() ? (
          <Markdown text={value} className="field-read" />
        ) : (
          <p className="field-empty">{placeholder}</p>
        )
      ) : (
        <>
          <textarea
            ref={ref}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={onKeyDown}
            rows={3}
            placeholder={placeholder}
          />
          <div className="field-hint muted">
            Markdown, and [[Card Name]] for a card. ⌘↵ saves, esc cancels.
          </div>
        </>
      )}
    </section>
  );
}

/** A saved engine at rest — description as prose, pieces as peekable chips. */
function EngineRow({
  engine,
  onEdit,
  onRemove,
}: {
  engine: Engine;
  onEdit: () => void;
  onRemove: () => void;
}) {
  const peekProps = usePeekProps();
  return (
    <div className="card-row">
      <div className="card-main">
        <span className="name">{engine.name}</span>
        <span className="muted">
          <CardText text={engine.description} />
        </span>
        <span className="spacer" />
        <button className="small" onClick={onEdit}>
          Edit
        </button>
        <button className="small danger" onClick={onRemove} aria-label={`Remove ${engine.name}`}>
          ✕
        </button>
      </div>
      <div className="card-tags">
        {engine.pieces.map((p) => (
          <span
            key={p.oracle_id}
            className={`tag-chip ${p.in_deck ? "active" : ""}`}
            {...peekProps(p.name)}
            title={`${p.name} — ${p.in_deck ? "in deck" : "NOT in deck"}; click for card text`}
          >
            {p.name}
            {!p.in_deck && " ⚠"}
          </span>
        ))}
      </div>
    </div>
  );
}

export function BriefPanel() {
  const { deckId, brief, apply } = useDeck();
  const [error, setError] = useState<string | null>(null);
  // null is the resting state here too: the engine form is a thing you open,
  // not three empty inputs sitting under the list you came to read.
  const [form, setForm] = useState<EngineForm | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.getBrief(deckId).then(apply);
  }, [deckId, apply]);

  if (!brief) return null;

  async function saveField(patch: { thesis?: string; constraints_md?: string }) {
    setError(null);
    try {
      apply(await api.updateBrief(deckId, patch));
    } catch (e: any) {
      setError(e.message);
      // Rethrown so the field keeps the draft it failed to save.
      throw e;
    }
  }

  async function submitEngine(e: FormEvent) {
    e.preventDefault();
    if (!form) return;
    setError(null);
    setBusy(true);
    try {
      const names = form.pieces.split(",").map((n) => n.trim()).filter(Boolean);
      const pieces: Array<{ oracle_id: string }> = [];
      for (const name of names) {
        const matches = await api.resolveCard(name);
        if (!matches.length) throw new Error(`'${name}' is not an exact card name`);
        pieces.push({ oracle_id: matches[0].oracle_id });
      }
      apply(await api.setEngine(deckId, form.name, form.description, pieces));
      setForm(null);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setBusy(false);
    }
  }

  // Chrome (title bar, collapse) belongs to the modal that hosts this.
  return (
    <>
      {error && <div className="error-banner">{error}</div>}

      <Field
        label="Thesis"
        value={brief.thesis}
        placeholder="What is this deck trying to do?"
        onSave={(thesis) => saveField({ thesis })}
      />
      <Field
        label="Constraints"
        value={brief.constraints_md}
        placeholder="Budget, pod meta, no infinite combos…"
        onSave={(constraints_md) => saveField({ constraints_md })}
      />

      <section className="brief-field">
        <div className="field-head">
          <h2>Named engines</h2>
          <span className="spacer" />
          {!form && (
            <button
              className="small"
              onClick={() => setForm({ name: "", description: "", pieces: "", existing: false })}
            >
              Add engine
            </button>
          )}
        </div>
        {!brief.engines.length && !form && (
          <p className="field-empty">No engines named yet — the combos this deck is actually built around.</p>
        )}
        {brief.engines.map((e) => (
          <EngineRow
            key={e.id}
            engine={e}
            onEdit={() =>
              setForm({
                name: e.name,
                description: e.description,
                pieces: e.pieces.map((p) => p.name).join(", "),
                existing: true,
              })
            }
            onRemove={() => api.removeEngine(deckId, e.id).then(apply).catch((err) => setError(err.message))}
          />
        ))}
        {form && (
          <form onSubmit={submitEngine} className="stack gap engine-form">
            <div className="row gap">
              <input
                value={form.name}
                onChange={(e) => setForm({ ...form, name: e.target.value })}
                placeholder="Engine name"
                autoFocus={!form.existing}
                // The name is the key the server upserts on, so editing through
                // a changed name would leave the original engine behind rather
                // than rename it. Remove and re-add to rename.
                readOnly={form.existing}
                title={form.existing ? "Remove and re-add the engine to rename it" : undefined}
              />
              <input
                className="grow"
                value={form.description}
                onChange={(e) => setForm({ ...form, description: e.target.value })}
                placeholder="Description"
                autoFocus={form.existing}
              />
            </div>
            <div className="row gap">
              <input
                className="grow"
                value={form.pieces}
                onChange={(e) => setForm({ ...form, pieces: e.target.value })}
                placeholder="Pieces: exact card names, comma-separated"
              />
              <button type="button" className="small" onClick={() => setForm(null)} disabled={busy}>
                Cancel
              </button>
              <button type="submit" className="primary" disabled={busy || !form.name.trim()}>
                {busy ? "Saving…" : form.existing ? "Save engine" : "Add engine"}
              </button>
            </div>
          </form>
        )}
      </section>
    </>
  );
}
