import type { ReactNode } from "react";
import type { LogEntry } from "../../api/client.ts";
import { ago } from "../../domain/time.ts";
import { CardRef, InlineText } from "../../mtg/index.ts";
import { Button, Text } from "../../ui/index.ts";
import { canUndo, documentEditLabel, isAgentAuthor } from "./logText.ts";
import styles from "./LogRow.module.css";

/** Past this many cards a sentence of names stops being readable; the entry's own summary takes over. */
const MAX_NAMED = 3;
/** A proposer's reason is shown under the row only when it is a line, not an essay. */
const SHORT_REASON = 140;

type LogCard = LogEntry["cards"][number];

type Lead = { word: string; tone: "add" | "cut" | "neutral" };

const LEAD: Record<LogEntry["kind"], Lead> = {
  accept: { word: "Accepted", tone: "add" },
  reject: { word: "Rejected", tone: "cut" },
  edit: { word: "Edited", tone: "neutral" },
  import: { word: "Imported", tone: "neutral" },
  undo: { word: "Undid", tone: "neutral" },
};

function names(cards: LogCard[]): ReactNode {
  const shown = cards.slice(0, MAX_NAMED);
  const more = cards.length - shown.length;
  return (
    <>
      {shown.map((c, i) => (
        <span key={`${c.oracle_id}-${c.action}`}>
          {i > 0 && (i === shown.length - 1 && !more ? " and " : ", ")}
          <CardRef name={c.card_name} />
        </span>
      ))}
      {more > 0 && ` and ${more} more`}
    </>
  );
}

function cardsPhrase(cards: LogCard[]): ReactNode {
  const parts: Array<[string, LogCard[]]> = [
    ["adding", cards.filter((c) => c.action === "add")],
    ["cutting", cards.filter((c) => c.action === "cut")],
    ["changing the role of", cards.filter((c) => c.action === "role")],
  ];
  const used = parts.filter(([, list]) => list.length > 0);
  return used.map(([verb, list], i) => (
    <span key={verb}>
      {i > 0 && " and "}
      {verb} {names(list)}
    </span>
  ));
}

/** Everything after the leading word. */
function describe(entry: LogEntry): ReactNode {
  const { kind, cards, summary } = entry;
  if (kind === "import") return summary || "a list";
  // Summaries are sentences of their own, so they follow a dash, not the verb directly.
  if (kind === "undo") return `— ${summary.replace(/^Undo of #\d+:\s*/, "")}`;
  if (entry.document_edit_id != null && cards.length === 0) {
    const label = documentEditLabel(summary);
    return label ? `the ${label.charAt(0).toLowerCase()}${label.slice(1)}` : summary;
  }
  if (cards.length > 0 && cards.length <= MAX_NAMED) return cardsPhrase(cards);
  // Many cards, or none: the entry's own summary says it better than a count.
  return summary ? `— ${summary}` : `${cards.length} cards`;
}

function CardReasons({ cards }: { cards: LogCard[] }) {
  const shown = cards.length <= 2 ? cards.filter((c) => c.reason && c.reason.length <= SHORT_REASON) : [];
  if (!shown.length) return null;
  return (
    <div className={styles.reasons}>
      {shown.map((c) => (
        <div key={`${c.oracle_id}-${c.action}`} className={styles.cardReason}>
          {cards.length > 1 && (
            <span className={styles.sign} aria-label={c.action === "cut" ? "cut" : "add"}>
              {c.action === "cut" ? "−" : "+"}
            </span>
          )}
          <InlineText text={c.reason} />
        </div>
      ))}
    </div>
  );
}

/** One entry of the rulings log (spec §8.2), compact enough to list dozens. */
export function LogRow({
  entry,
  onUndo,
  showDeck = false,
}: {
  entry: LogEntry;
  onUndo?: (entry: LogEntry) => void;
  showDeck?: boolean;
}) {
  const lead = LEAD[entry.kind];
  const unread = entry.reviewed_at == null;
  const undone = entry.undone_by != null;
  const deck = entry.deck_name ?? "Your profile";

  return (
    <div className={styles.row}>
      <Text mono size="xs" tone="faint" className={styles.rev}>
        r{entry.revision}
      </Text>
      <div className={styles.what}>
        <span className={styles[lead.tone]}>{lead.word}</span> <span className={undone ? styles.dim : undefined}>{describe(entry)}</span>
        {unread && (
          <span
            className={styles.unread}
            title="No agent has read this ruling yet. The next agent session will."
          >
            unread by agents
          </span>
        )}
        {undone && <span className={styles.undone}>undone</span>}
      </div>
      <div className={styles.action}>
        {onUndo && canUndo(entry) && (
          <Button variant="ghost" size="sm" onClick={() => onUndo(entry)}>
            Undo
          </Button>
        )}
      </div>
      <div className={styles.meta}>
        {entry.ruling_reason && <q className={styles.quote}>{entry.ruling_reason}</q>}
        <CardReasons cards={entry.cards} />
        <div className={styles.faint}>
          <span className={isAgentAuthor(entry.author) ? styles.agentAuthor : undefined}>{entry.author}</span>
          {entry.proposal_id != null && ` · via proposal ${entry.proposal_id}`}
          {entry.document_edit_id != null && ` · via edit ${entry.document_edit_id}`}
          {` · ${ago(entry.ts)}`}
          {showDeck && ` · ${deck}`}
        </div>
      </div>
    </div>
  );
}
