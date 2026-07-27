import { useState, type FormEvent } from "react";
import { api, type DeckState } from "./api.ts";

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

  return (
    <aside className="slot-panel">
      <h2>Slots</h2>
      <ul className="slot-list">
        {computed.slot_deltas.map((d) => (
          <li key={d.slot_id} className={d.status}>
            <span className="slot-name">{d.name}</span>
            <button
              className="link slot-target"
              title="Click to edit target"
              onClick={() => editTargets(d.slot_id, d.target_min, d.target_max)}
            >
              {d.count}
              {(d.target_min != null || d.target_max != null) &&
                ` / ${d.target_min ?? 0}–${d.target_max ?? "∞"}`}
            </button>
            <span className={`status ${d.status}`}>
              {statusSymbol[d.status]}
              {d.delta !== 0 && Math.abs(d.delta)}
            </span>
            <button
              className="small danger"
              title="Delete slot (cards become unslotted)"
              onClick={() => mutate(() => api.deleteSlot(deck.id, d.slot_id))}
            >
              ✕
            </button>
          </li>
        ))}
        <li className="muted">
          <span className="slot-name">Unslotted</span>
          <span>{computed.unslotted_count}</span>
        </li>
      </ul>
      <form onSubmit={addSlot} className="stack gap">
        <input
          value={slotName}
          onChange={(e) => setSlotName(e.target.value)}
          placeholder="New slot name"
        />
        <div className="row gap">
          <input
            value={slotMin}
            onChange={(e) => setSlotMin(e.target.value)}
            placeholder="min"
            size={4}
          />
          <input
            value={slotMax}
            onChange={(e) => setSlotMax(e.target.value)}
            placeholder="max"
            size={4}
          />
          <button type="submit" disabled={!slotName.trim()}>
            Add slot
          </button>
        </div>
      </form>

      <h2>Tags</h2>
      <div className="tag-cloud">
        {tags.map((t) => (
          <span key={t.id} className="tag-chip">
            {t.name}
            <button
              className="link danger"
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
        <input value={tagName} onChange={(e) => setTagName(e.target.value)} placeholder="New tag" />
        <button type="submit" disabled={!tagName.trim()}>
          Add
        </button>
      </form>

      <h2>Curve</h2>
      <div className="curve">
        {Object.entries(computed.curve).map(([bucket, n]) => (
          <div key={bucket} className="curve-col" title={`${n} card(s) at MV ${bucket}`}>
            <div className="curve-bar" style={{ height: `${Math.min(n * 6, 90)}px` }} />
            <span className="muted">{bucket}</span>
            <span>{n}</span>
          </div>
        ))}
      </div>
      <div className="muted">
        Pips:{" "}
        {Object.entries(computed.pips)
          .filter(([, n]) => n > 0)
          .map(([c, n]) => `${c}${n}`)
          .join(" ") || "—"}
      </div>
    </aside>
  );
}
