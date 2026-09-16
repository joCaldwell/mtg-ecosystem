import { useEffect, useState, type MouseEvent } from "react";
import { api, type DeckCard } from "./api.ts";
import { useDeck } from "./store.tsx";
import { ptString } from "./lib.ts";
import { ManaCost } from "./Mana.tsx";

export type OwnershipMode = "owned" | "missing";

export function CardRow({
  card,
  ownershipMode,
}: {
  card: DeckCard;
  ownershipMode: OwnershipMode;
}) {
  const { deckId, state, run } = useDeck();
  const { slots, tags } = state!;
  const [expanded, setExpanded] = useState(false);
  const [quantityDraft, setQuantityDraft] = useState(String(card.quantity));

  useEffect(() => setQuantityDraft(String(card.quantity)), [card.quantity]);

  const shortType = card.type_line.split("—")[0].trim();
  const pt = ptString(card);

  function toggleTag(tagId: number) {
    const next = card.tag_ids.includes(tagId)
      ? card.tag_ids.filter((t) => t !== tagId)
      : [...card.tag_ids, tagId];
    run(() => api.updateCard(deckId, card.oracle_id, { tag_ids: next }));
  }

  // Only the tags actually on this card ride along in the row; the full cloud
  // of toggles lives in the expanded panel, where it isn't repeated 100 times.
  const activeTags = tags.filter((t) => card.tag_ids.includes(t.id));

  // The whole row toggles the detail panel, but the row also carries its own
  // controls — the ownership checkbox, slot select, action buttons, and the
  // name button that keeps this reachable from the keyboard. Anything
  // interactive handles its own click, so the row bows out.
  function toggleFromRow(e: MouseEvent<HTMLDivElement>) {
    if ((e.target as HTMLElement).closest("button, select, input, label, a")) return;
    setExpanded(!expanded);
  }

  const markingMissing = ownershipMode === "missing";
  const ownershipMarked = markingMissing ? !card.owned : !!card.owned;

  async function commitQuantity() {
    const value = quantityDraft.trim();
    const quantity = Number(value);
    if (!/^\d+$/.test(value) || quantity < 1) {
      setQuantityDraft(String(card.quantity));
      return;
    }
    setQuantityDraft(String(quantity));
    if (quantity === card.quantity) return;
    const result = await run(() => api.updateCard(deckId, card.oracle_id, { quantity }));
    if (!result) setQuantityDraft(String(card.quantity));
  }

  return (
    <div
      className={`card-row ${expanded ? "is-open" : ""} ${markingMissing && !card.owned ? "is-missing" : ""}`}
    >
      <div className="card-main is-clickable" onClick={toggleFromRow}>
        {/* Name and cost share one growing box so the name keeps its natural
            width — a bare flex spacer would shrink it into an ellipsis first. */}
        <span className="card-id">
          <button className="link name" onClick={() => setExpanded(!expanded)}>
            {card.name}
          </button>
          <ManaCost cost={card.mana_cost} />
        </span>

        {/* Type sits on the right so it forms a straight column; names vary in
            length and would otherwise scatter it across the row. */}
        <span className="type">
          {shortType}
          {pt && <span className="pt"> {pt}</span>}
        </span>

        {activeTags.length > 0 && (
          <span className="row-tags">
            {activeTags.map((t) => (
              <span key={t.id} className="tag-chip active static">
                {t.name}
              </span>
            ))}
          </span>
        )}

        <label
          className={`own-toggle ${markingMissing ? "is-missing" : ""}`}
          title={`${markingMissing ? "Missing" : "Owned"} (never shown to the agent)`}
        >
          <input
            type="checkbox"
            checked={ownershipMarked}
            onChange={(e) =>
              run(() =>
                api.updateCard(deckId, card.oracle_id, {
                  owned: markingMissing ? !e.target.checked : e.target.checked,
                }),
              )
            }
          />
          {markingMissing ? "missing" : "own"}
        </label>

        <select
          className="ghost"
          value={card.slot_id ?? ""}
          onChange={(e) =>
            run(() =>
              api.updateCard(deckId, card.oracle_id, {
                slot_id: e.target.value === "" ? null : Number(e.target.value),
              }),
            )
          }
          title="Slot"
        >
          <option value="">unslotted</option>
          {slots.map((s) => (
            <option key={s.id} value={s.id}>
              {s.name}
            </option>
          ))}
        </select>

        <label
          className={`quantity-control ${card.quantity > 1 ? "has-multiple" : ""}`}
          title="Copies — type a quantity"
        >
          <span aria-hidden="true">×</span>
          <input
            type="text"
            inputMode="numeric"
            value={quantityDraft}
            onChange={(e) => setQuantityDraft(e.target.value)}
            onFocus={(e) => e.currentTarget.select()}
            onBlur={() => void commitQuantity()}
            onKeyDown={(e) => {
              if (e.key === "Enter") e.currentTarget.blur();
              if (e.key === "Escape") setQuantityDraft(String(card.quantity));
            }}
            aria-label={`Quantity for ${card.name}`}
            spellCheck={false}
          />
        </label>

        <span className="row-actions">
          <button
            className="icon danger"
            title={card.quantity > 1 ? "Remove all copies" : "Remove card"}
            onClick={() => run(() => api.removeCard(deckId, card.oracle_id))}
          >
            ✕
          </button>
        </span>
      </div>

      {expanded && (
        <div className="card-detail">
          {card.oracle_text && <pre className="oracle-text">{card.oracle_text}</pre>}
          <div className="row gap wrap">
            <label className="field">
              Role
              <select
                value={card.role}
                onChange={(e) =>
                  run(() => api.updateCard(deckId, card.oracle_id, { role: e.target.value }))
                }
              >
                <option value="card">card</option>
                <option value="commander">commander</option>
                <option value="companion">companion</option>
              </select>
            </label>
            {tags.length > 0 && (
              <div className="tag-cloud" style={{ marginBottom: 0 }}>
                {tags.map((t) => (
                  <button
                    key={t.id}
                    className={`tag-chip ${card.tag_ids.includes(t.id) ? "active" : ""}`}
                    onClick={() => toggleTag(t.id)}
                  >
                    {t.name}
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
