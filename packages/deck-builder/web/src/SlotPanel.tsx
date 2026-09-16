import { useState, type FormEvent } from "react";
import { api } from "./api.ts";
import { useDeck } from "./store.tsx";
import { faceOf } from "./Mana.tsx";
import { Modal } from "./Modal.tsx";
import { SidebarResize } from "./SidebarResize.tsx";

type TargetEdit = { slotId: number; name: string };

export function SlotPanel() {
  const { state, run } = useDeck();
  const { deck, tags, cards, computed } = state!;
  const [slotName, setSlotName] = useState("");
  const [slotMin, setSlotMin] = useState("");
  const [slotMax, setSlotMax] = useState("");
  const [tagName, setTagName] = useState("");
  const [targetEdit, setTargetEdit] = useState<TargetEdit | null>(null);
  const [targetValue, setTargetValue] = useState("");
  const [targetError, setTargetError] = useState<string | null>(null);
  const [targetSaving, setTargetSaving] = useState(false);

  function addSlot(e: FormEvent) {
    e.preventDefault();
    const min = slotMin === "" ? null : Number(slotMin);
    // A single number is shorthand for min = max
    const max = slotMax === "" ? (slotMin === "" ? null : Number(slotMin)) : Number(slotMax);
    run(() => api.createSlot(deck.id, slotName, min, max)).then(() => {
      setSlotName("");
      setSlotMin("");
      setSlotMax("");
    });
  }

  function editTargets(
    slotId: number,
    name: string,
    currentMin: number | null,
    currentMax: number | null,
  ) {
    const exact = currentMin != null && currentMin === currentMax;
    setTargetValue(
      currentMin == null && currentMax == null
        ? ""
        : exact
          ? String(currentMin)
          : `${currentMin ?? ""}-${currentMax ?? ""}`,
    );
    setTargetError(null);
    setTargetEdit({ slotId, name });
  }

  async function saveTargets(e: FormEvent) {
    e.preventDefault();
    if (!targetEdit) return;
    const trimmed = targetValue.trim();
    const range = trimmed.match(/^(?:\d+\s*-\s*\d*|\s*-\s*\d+)$/);
    const exact = trimmed.match(/^(\d+)$/);
    if (!range && !exact) {
      setTargetError("Use a number or range, such as 10, 8-12, 8-, or -12.");
      return;
    }
    const [lower = "", upper = ""] = exact ? [exact[1], exact[1]] : trimmed.split(/\s*-\s*/, 2);
    const min = lower === "" ? null : Number(lower);
    const max = upper === "" ? null : Number(upper);
    if (min != null && max != null && min > max) {
      setTargetError("The minimum cannot be greater than the maximum.");
      return;
    }
    setTargetSaving(true);
    const result = await run(() =>
      api.updateSlot(deck.id, targetEdit.slotId, { target_min: min, target_max: max }),
    );
    setTargetSaving(false);
    if (result) setTargetEdit(null);
  }

  async function clearTargets() {
    if (!targetEdit) return;
    setTargetSaving(true);
    const result = await run(() =>
      api.updateSlot(deck.id, targetEdit.slotId, { target_min: null, target_max: null }),
    );
    setTargetSaving(false);
    if (result) setTargetEdit(null);
  }

  function addTag(e: FormEvent) {
    e.preventDefault();
    run(() => api.createTag(deck.id, tagName)).then(() => setTagName(""));
  }

  const statusSymbol = { ok: "✓", under: "▼", over: "▲", untargeted: "" };

  const curve = Object.entries(computed.curve);
  const curveMax = Math.max(...curve.map(([, n]) => n), 1);
  // Ownership stays in the UI: it must never enter the agent's computed state.
  const countedCards = cards.filter((c) => c.role !== "companion" && !c.maybeboard);
  const owned = countedCards.reduce((n, c) => n + (c.owned ? c.quantity : 0), 0);
  const remaining = 100 - computed.card_count;

  return (
    <aside className="slot-panel">
      <SidebarResize side="left" />
      <div className="slot-body">
      <section className="deck-overview" aria-labelledby="deck-overview-heading">
        <h2 id="deck-overview-heading">Deck overview</h2>
        <div className="overview-total">
          <strong>{computed.card_count}<span> / 100</span></strong>
          <span className={`status ${remaining === 0 ? "ok" : remaining > 0 ? "under" : "over"}`}>
            {remaining === 0 ? "Complete" : remaining > 0 ? `${remaining} to add` : `${-remaining} to cut`}
          </span>
        </div>
        <div className="overview-progress" aria-hidden="true">
          <i style={{ width: `${Math.min(100, computed.card_count)}%` }} />
        </div>
        <dl className="overview-stats">
          <div><dt>Lands</dt><dd>{computed.land_count}</dd></div>
          <div title="Average mana value of nonland cards, including commanders; excludes companion and maybe list.">
            <dt>Avg. MV <span>(nonland)</span></dt><dd>{computed.average_mana_value?.toFixed(2) ?? "—"}</dd>
          </div>
          <div><dt>Owned</dt><dd>{owned}</dd></div>
          <div><dt>Missing</dt><dd>{computed.card_count - owned}</dd></div>
        </dl>
        {(computed.pending.adds > 0 || computed.pending.cuts > 0) && (
          <p className="overview-pending">
            Pending <span>+{computed.pending.adds} / −{computed.pending.cuts}</span>
            <span>→ {computed.pending.projected_count} cards</span>
          </p>
        )}
      </section>
      <h2>Slots</h2>
      <ul className="slot-list">
        {computed.slot_deltas.map((d) => {
          // Fill against the upper bound so "how close am I" reads at a glance;
          // an untargeted slot gets no bar rather than a meaningless full one.
          const ceiling = d.target_max ?? d.target_min;
          const fill = ceiling ? Math.min(100, (d.count / ceiling) * 100) : null;
          return (
            <li key={d.slot_id} className={d.status}>
              <span className="slot-name" title={d.name}>
                {d.name}
              </span>
              <button
                className="link slot-target"
                title="Click to edit target"
                onClick={() => editTargets(d.slot_id, d.name, d.target_min, d.target_max)}
                aria-haspopup="dialog"
              >
                {d.count}
                {(d.target_min != null || d.target_max != null) &&
                  ` / ${d.target_min ?? 0}–${d.target_max ?? "∞"}`}
              </button>
              <span className="row">
                <span className={`status ${d.status}`}>
                  {statusSymbol[d.status]}
                  {d.delta !== 0 && Math.abs(d.delta)}
                </span>
                <button
                  className="icon danger del"
                  title="Delete slot (cards become unslotted)"
                  onClick={() => run(() => api.deleteSlot(deck.id, d.slot_id))}
                >
                  ✕
                </button>
              </span>
              {fill != null && (
                <span className="slot-meter">
                  <i style={{ width: `${fill}%` }} />
                </span>
              )}
            </li>
          );
        })}
        <li className="unslotted">
          <span className="slot-name">Unslotted</span>
          <span className="slot-target">{computed.unslotted_count}</span>
          <span />
        </li>
      </ul>

      <form onSubmit={addSlot} className="stack gap slot-form">
        <input
          value={slotName}
          onChange={(e) => setSlotName(e.target.value)}
          placeholder="New slot name"
        />
        <div className="row gap">
          <input
            className="num grow"
            value={slotMin}
            onChange={(e) => setSlotMin(e.target.value)}
            placeholder="min"
          />
          <input
            className="num grow"
            value={slotMax}
            onChange={(e) => setSlotMax(e.target.value)}
            placeholder="max"
          />
          <button type="submit" disabled={!slotName.trim()}>
            Add
          </button>
        </div>
      </form>

      <h2>Tags</h2>
      <div className="tag-cloud">
        {tags.map((t) => (
          <span key={t.id} className="tag-chip static">
            {t.name}
            <button
              className="link del"
              title="Delete tag"
              onClick={() => run(() => api.deleteTag(deck.id, t.id))}
            >
              ✕
            </button>
          </span>
        ))}
        {!tags.length && <span className="muted">none yet</span>}
      </div>
      <form onSubmit={addTag} className="row gap">
        <input
          className="grow"
          value={tagName}
          onChange={(e) => setTagName(e.target.value)}
          placeholder="New tag"
        />
        <button type="submit" disabled={!tagName.trim()}>
          Add
        </button>
      </form>

      <h2>Curve</h2>
      <div className="curve">
        {curve.map(([bucket, n]) => (
          <div key={bucket} className="curve-col" title={`${n} card(s) at MV ${bucket}`}>
            <span>{n || ""}</span>
            <div className="curve-bar" style={{ height: `${(n / curveMax) * 100}%` }} />
          </div>
        ))}
      </div>
      <div className="curve-axis">
        {curve.map(([bucket]) => (
          <span key={bucket}>{bucket}</span>
        ))}
      </div>

      <div className="pips-line">
        {Object.entries(computed.pips)
          .filter(([, n]) => n > 0)
          .map(([c, n]) => (
            <span key={c} className="pip-count" title={`${n} ${c} pip(s)`}>
              <i
                className="pip"
                style={{ background: faceOf(c).bg, color: faceOf(c).fg }}
              >
                {c}
              </i>
              {n}
            </span>
          ))}
        {!Object.values(computed.pips).some((n) => n > 0) && <span className="muted">no pips</span>}
      </div>
      </div>
      {targetEdit && (
        <Modal
          title={`Target · ${targetEdit.name}`}
          onClose={() => {
            if (!targetSaving) setTargetEdit(null);
          }}
        >
          <form className="stack gap" onSubmit={saveTargets}>
            <label className="field-label" htmlFor="slot-target-value">
              Target count or range
            </label>
            <input
              id="slot-target-value"
              value={targetValue}
              onChange={(e) => {
                setTargetValue(e.target.value);
                setTargetError(null);
              }}
              onFocus={(e) => e.currentTarget.select()}
              placeholder="10 or 8-12"
              aria-describedby="slot-target-help"
              aria-invalid={targetError != null}
              data-modal-autofocus
              disabled={targetSaving}
            />
            <div id="slot-target-help" className={targetError ? "field-error" : "muted rationale"}>
              {targetError ?? "A single number is exact. Use 8- or -12 for an open-ended range."}
            </div>
            <div className="dialog-actions">
              <button type="button" onClick={() => void clearTargets()} disabled={targetSaving}>
                Clear target
              </button>
              <span className="spacer" />
              <button type="button" onClick={() => setTargetEdit(null)} disabled={targetSaving}>
                Cancel
              </button>
              <button className="primary" type="submit" disabled={targetSaving || !targetValue.trim()}>
                {targetSaving ? "Saving…" : "Save target"}
              </button>
            </div>
          </form>
        </Modal>
      )}
    </aside>
  );
}
