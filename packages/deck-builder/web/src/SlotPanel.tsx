import { useState, type FormEvent } from "react";
import { api, type DeckState } from "./api.ts";

const PIP_FACE: Record<string, string> = {
  W: "#f4eeda",
  U: "#3f92d2",
  B: "#4c4653",
  R: "#dc6a52",
  G: "#46a06b",
  C: "#b3b9c3",
};
const PIP_TEXT: Record<string, string> = {
  W: "#2c2617",
  U: "#04121e",
  B: "#e0dae6",
  R: "#2a0f09",
  G: "#062011",
  C: "#191c22",
};

export function SlotPanel({
  state,
  mutate,
}: {
  state: DeckState;
  mutate: (fn: () => Promise<DeckState>) => Promise<void>;
}) {
  const { deck, slots, tags, computed } = state;
  const [slotName, setSlotName] = useState("");
  const [slotMin, setSlotMin] = useState("");
  const [slotMax, setSlotMax] = useState("");
  const [tagName, setTagName] = useState("");

  function addSlot(e: FormEvent) {
    e.preventDefault();
    const min = slotMin === "" ? null : Number(slotMin);
    // A single number is shorthand for min = max
    const max = slotMax === "" ? (slotMin === "" ? null : Number(slotMin)) : Number(slotMax);
    mutate(() => api.createSlot(deck.id, slotName, min, max)).then(() => {
      setSlotName("");
      setSlotMin("");
      setSlotMax("");
    });
  }

  function editTargets(slotId: number, currentMin: number | null, currentMax: number | null) {
    const raw = window.prompt(
      "Target (e.g. '10' or '8-12', empty to clear)",
      currentMin != null || currentMax != null ? `${currentMin ?? ""}-${currentMax ?? ""}` : "",
    );
    if (raw == null) return;
    const trimmed = raw.trim();
    if (!trimmed) {
      mutate(() => api.updateSlot(deck.id, slotId, { target_min: null, target_max: null }));
      return;
    }
    const m = trimmed.match(/^(\d+)?\s*-\s*(\d+)?$/) ?? trimmed.match(/^(\d+)$/);
    if (!m) return;
    const min = m[1] != null ? Number(m[1]) : null;
    const max = m.length > 2 && m[2] != null ? Number(m[2]) : m.length > 2 ? null : min;
    mutate(() => api.updateSlot(deck.id, slotId, { target_min: min, target_max: max }));
  }

  function addTag(e: FormEvent) {
    e.preventDefault();
    mutate(() => api.createTag(deck.id, tagName)).then(() => setTagName(""));
  }

  const statusSymbol = { ok: "✓", under: "▼", over: "▲", untargeted: "" };

  const curve = Object.entries(computed.curve);
  const curveMax = Math.max(...curve.map(([, n]) => n), 1);

  return (
    <aside className="slot-panel">
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
                onClick={() => editTargets(d.slot_id, d.target_min, d.target_max)}
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
                  onClick={() => mutate(() => api.deleteSlot(deck.id, d.slot_id))}
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
              onClick={() => mutate(() => api.deleteTag(deck.id, t.id))}
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
                style={{ background: PIP_FACE[c] ?? "#b3b9c3", color: PIP_TEXT[c] ?? "#191c22" }}
              >
                {c}
              </i>
              {n}
            </span>
          ))}
        {!Object.values(computed.pips).some((n) => n > 0) && <span className="muted">no pips</span>}
      </div>
    </aside>
  );
}
