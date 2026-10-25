import type { CardRuling } from "../../api/client.ts";
import { ago } from "../../domain/time.ts";
import { InlineText } from "../../mtg/index.ts";
import { Text } from "../../ui/index.ts";
import styles from "./CardRulingRow.module.css";

/** What happened to this one card, as a clause that follows "r95 · ". */
function clause(r: CardRuling): string {
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

/** One ruling about one card, for the card's panel and the review context (spec §8.3). */
export function CardRulingRow({ ruling }: { ruling: CardRuling }) {
  return (
    <div className={styles.row}>
      <div className={styles.line}>
        <Text mono size="xs" tone="faint">
          r{ruling.revision}
        </Text>
        {" · "}
        <span className={ruling.kind === "reject" ? styles.cut : ruling.kind === "accept" ? styles.add : undefined}>
          {clause(ruling)}
        </span>
        {ruling.ruling_reason && <q className={styles.quote}>{ruling.ruling_reason}</q>}
      </div>
      {ruling.reason && (
        <div className={styles.reason}>
          <InlineText text={ruling.reason} />
        </div>
      )}
      <div className={styles.meta}>
        {ruling.author_display ?? ruling.author} · {ago(ruling.ts)}
        {ruling.undone && " · undone"}
        {ruling.from_deck != null && ` · in ${ruling.from_deck}`}
      </div>
    </div>
  );
}
