import { useState } from "react";
import { api, type DeckCard, type DeckState, type DraftItem, type Slot, type Tag } from "./api.ts";

export function CardRow({
  card,
  deckId,
  slots,
  tags,
  mutate,
  draftAdd,
}: {
  card: DeckCard;
  deckId: number;
  slots: Slot[];
  tags: Tag[];
  mutate: (fn: () => Promise<DeckState>) => Promise<void>;
  draftAdd?: (item: DraftItem) => void;
}) {
  const [expanded, setExpanded] = useState(false);

  const shortType = card.type_line.split("—")[0].trim();
  const pt =
    card.power != null && card.toughness != null
      ? `${card.power}/${card.toughness}`
      : card.loyalty != null
        ? `[${card.loyalty}]`
        : "";

  function toggleTag(tagId: number) {
    const next = card.tag_ids.includes(tagId)
      ? card.tag_ids.filter((t) => t !== tagId)
      : [...card.tag_ids, tagId];
    mutate(() => api.updateCard(deckId, card.oracle_id, { tag_ids: next }));
  }

  return (
    <div className="card-row">
      <div className="card-main">
        <span className="qty">
          {card.quantity > 1 && `${card.quantity}× `}
        </span>
        <button className="link name" onClick={() => setExpanded(!expanded)}>
          {card.name}
        </button>
        <span className="mono cost">{card.mana_cost ?? ""}</span>
        <span className="muted type">
          {shortType}
          {pt && ` ${pt}`}
        </span>

        <span className="spacer" />

        <label className="owned" title="Owned (never shown to the agent)">
          <input
            type="checkbox"
            checked={!!card.owned}
            onChange={(e) =>
              mutate(() => api.updateCard(deckId, card.oracle_id, { owned: e.target.checked }))
            }
          />
          own
        </label>

        <select
          value={card.slot_id ?? ""}
          onChange={(e) =>
            mutate(() =>
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

        <select
          value={card.role}
          onChange={(e) => mutate(() => api.updateCard(deckId, card.oracle_id, { role: e.target.value }))}
          title="Role"
        >
          <option value="card">card</option>
          <option value="commander">commander</option>
          <option value="companion">companion</option>
        </select>

        <span className="qty-controls">
          {draftAdd && (
            <button
              className="small"
              title="Propose cutting this card"
              onClick={() =>
                draftAdd({ action: "cut", oracle_id: card.oracle_id, card_name: card.name, rationale: "" })
              }
            >
              ✂
            </button>
          )}
          <button
            className="small"
            title="Remove one / remove card"
            onClick={() =>
              card.quantity > 1
                ? mutate(() =>
                    api.updateCard(deckId, card.oracle_id, { quantity: card.quantity - 1 }),
                  )
                : mutate(() => api.removeCard(deckId, card.oracle_id))
            }
          >
            −
          </button>
          <button
            className="small"
            title="Add a copy"
            onClick={() =>
              mutate(() => api.updateCard(deckId, card.oracle_id, { quantity: card.quantity + 1 }))
            }
          >
            +
          </button>
          <button
            className="small danger"
            title="Remove card entirely"
            onClick={() => mutate(() => api.removeCard(deckId, card.oracle_id))}
          >
            ✕
          </button>
        </span>
      </div>

      {tags.length > 0 && (
        <div className="card-tags">
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

      {expanded && <pre className="oracle-text">{card.oracle_text}</pre>}
    </div>
  );
}
