import { usePreferences } from "../../app/Preferences.tsx";
import type { CardDecision } from "../../api/client.ts";
import { ago } from "../../domain/time.ts";
import { InlineText } from "../../mtg/index.ts";
import { Text } from "../../ui/index.ts";
import styles from "./CardDecisionRow.module.css";

/** What happened to this one card, as a clause that follows "r95 · ". */
function clause(r: CardDecision): string {
  const who = r.author_via === "agent" ? (r.author_display ?? r.author) : "you";
  const verb = r.action === "add" ? "adding" : r.action === "cut" ? "cutting" : "changing the role of";
  switch (r.kind) {
    case "accept":
      return `you accepted ${verb} it`;
    case "reject":
      return `you rejected ${verb} it`;
    case "import":
      return r.action === "role" ? "an import changed its role" : `an import ${r.action === "add" ? "added" : "cut"} it`;
    case "undo":
      return r.action === "add" ? "an undo put it back" : r.action === "cut" ? "an undo took it out" : "an undo restored its role";
    case "edit":
      return r.action === "role" ? `${who} changed its role` : `${who} ${r.action === "add" ? "added" : "cut"} it`;
  }
}

/** One decision about one card, for the card's panel and the review context (spec §8.3). */
export function CardDecisionRow({ decision }: { decision: CardDecision }) {
  const { advanced } = usePreferences();
  return (
    <div className={styles.row}>
      <div className={styles.line}>
        {advanced && <><Text mono size="xs" tone="faint">
          r{decision.revision}
        </Text>
        {" · "}</>}
        <span className={decision.kind === "reject" ? styles.cut : decision.kind === "accept" ? styles.add : undefined}>
          {clause(decision)}
        </span>
        {decision.decision_reason && <q className={styles.quote}>{decision.decision_reason}</q>}
      </div>
      {decision.reason && (
        <div className={styles.reason}>
          <InlineText text={decision.reason} />
        </div>
      )}
      <div className={styles.meta}>
        {decision.author_display ?? decision.author} · {ago(decision.ts)}
        {decision.undone && " · undone"}
        {decision.from_deck != null && ` · in ${decision.from_deck}`}
      </div>
    </div>
  );
}
