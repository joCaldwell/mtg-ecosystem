import { useState, type MouseEvent } from "react";
import { api, type DeckCard, type DraftItem } from "./api.ts";
import { useDeck } from "./store.tsx";
import { ptString } from "./lib.ts";
import { ManaCost } from "./Mana.tsx";

export function CardRow({
  card,
  draftAdd,
}: {
  card: DeckCard;
  draftAdd?: (item: DraftItem) => void;
}) {
  const { deckId, state, run } = useDeck();
  const { slots, tags } = state!;
  const [expanded, setExpanded] = useState(false);

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
  // controls — the own checkbox, the slot select, the action buttons, and the
  // name button that keeps this reachable from the keyboard. Anything
  // interactive handles its own click, so the row bows out.
  function toggleFromRow(e: MouseEvent<HTMLDivElement>) {
    if ((e.target as HTMLElement).closest("button, select, input, label, a")) return;
    setExpanded(!expanded);
  }

  return (
    <div className={`card-row ${expanded ? "is-open" : ""}`}>
      <div className="card-main is-clickable" onClick={toggleFromRow}>
        {/* Name and cost share one growing box so the name keeps its natural
            width — a bare flex spacer would shrink it into an ellipsis first.
            The count follows the name rather than leading it: singletons are
            the overwhelming majority, and a reserved gutter left every one of
            them indented behind an empty column. */}
        <span className="card-id">
          <button className="link name" onClick={() => setExpanded(!expanded)}>
            {card.name}
          </button>
          {card.quantity > 1 && <span className="qty">×{card.quantity}</span>}
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

        <label className="own-toggle" title="Owned (never shown to the agent)">
          <input
            type="checkbox"
            checked={!!card.owned}
            onChange={(e) =>
              run(() => api.updateCard(deckId, card.oracle_id, { owned: e.target.checked }))
            }
          />
          own
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

        <span className="row-actions">
          {draftAdd && (
            <button
              className="icon"
              title="Propose cutting this card"
              onClick={() =>
                draftAdd({
                  action: "cut",
                  oracle_id: card.oracle_id,
                  card_name: card.name,
                  rationale: "",
                })
              }
            >
              ✂
            </button>
          )}
          <button
            className="icon"
            title="Remove one / remove card"
            onClick={() =>
              card.quantity > 1
                ? run(() =>
                    api.updateCard(deckId, card.oracle_id, { quantity: card.quantity - 1 }),
                  )
                : run(() => api.removeCard(deckId, card.oracle_id))
            }
          >
            −
          </button>
          <button
            className="icon"
            title="Add a copy"
            onClick={() =>
              run(() => api.updateCard(deckId, card.oracle_id, { quantity: card.quantity + 1 }))
            }
          >
            +
          </button>
          <button
            className="icon danger"
            title="Remove card entirely"
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
