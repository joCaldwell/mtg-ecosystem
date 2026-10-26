// A saved engine (spec §6.2): a named combo or plan, with its pieces checked
// against the list as it is now.

import type { Engine } from "../../../api/client.ts";
import { RichText, useCardHover } from "../../../mtg/index.ts";
import { Box, Button, Chip } from "../../../ui/index.ts";
import { useDeckList } from "../DeckStore.tsx";
import { useDeckUi } from "../DeckUi.tsx";
import styles from "./EngineCard.module.css";

/** Without onEdit and onRemove it is read-only: a shared deck or the phone view. */
export function EngineCard({ engine, onEdit, onRemove }: { engine: Pick<Engine, "name" | "description" | "pieces">; onEdit?: () => void; onRemove?: () => void }) {
  const { state } = useDeckList();
  const ui = useDeckUi();
  const hover = useCardHover();
  const missing = engine.pieces.filter((p) => !p.in_deck).length;

  return (
    <Box>
      <div className={styles.head}>
        <b className={styles.name}>{engine.name}</b>
        {missing > 0 ? <Chip tone="warn">{missing} missing</Chip> : <Chip tone="add">complete</Chip>}
        {onEdit && onRemove && <span className={styles.actions}>
          <Button size="sm" variant="ghost" disabled={state.deck.category === "legacy"} onClick={onEdit} aria-label={`Edit the engine ${engine.name}`}>
            Edit
          </Button>
          <Button size="sm" variant="ghost" disabled={state.deck.category === "legacy"} onClick={onRemove} aria-label={`Remove the engine ${engine.name}`}>
            Remove
          </Button>
        </span>}
      </div>
      {engine.description.trim() && <RichText text={engine.description} className={styles.description} />}
      {engine.pieces.length > 0 && (
        <div className={styles.pieces}>
          {engine.pieces.map((p) => {
            const parked = !p.in_deck && state.cards.some((c) => c.oracle_id === p.oracle_id && c.maybe);
            return (
              <Chip
                key={p.oracle_id}
                tone={p.in_deck ? "neutral" : "warn"}
                onClick={() => ui.openCardPanel({ oracleId: p.oracle_id, name: p.name })}
                {...hover(p.name)}
              >
                {p.in_deck ? "✓" : "✗"} {p.name}
                {parked && " · on maybe list"}
              </Chip>
            );
          })}
        </div>
      )}
    </Box>
  );
}
