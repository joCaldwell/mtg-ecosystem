import { editTitle } from "../../domain/documents.ts";
import { reviewUnits } from "../../domain/review.ts";
import { ago } from "../../domain/time.ts";
import type { MouseEvent } from "react";
import { Chip } from "../../ui/index.ts";
import { describeEdit, plainText, type Entry } from "./entries.ts";
import styles from "./InboxEntry.module.css";

function parts(entry: Entry) {
  if (entry.type === "proposal") {
    const { proposal } = entry;
    const units = reviewUnits([proposal]);
    const changes = units.reduce((n, u) => n + u.items.length, 0);
    return {
      chip: "Proposal",
      where: proposal.deck_name,
      author: proposal.author_display ?? proposal.author,
      title: proposal.summary ? plainText(proposal.summary) : "No summary given",
      sub: `${changes} ${changes === 1 ? "change" : "changes"} in ${units.length} ${units.length === 1 ? "decision" : "decisions"}`,
    };
  }
  const { edit } = entry;
  return {
    chip: edit.kind === "profile" ? "Profile edit" : "Brief edit",
    where: edit.deck_name ?? "Your profile",
    author: edit.author_display ?? edit.author,
    title: editTitle(edit),
    sub: describeEdit(edit),
  };
}

/** One row of the queue. */
export function InboxEntry({ entry, selected, onSelect, href }: { entry: Entry; selected: boolean; onSelect: () => void; href?: string }) {
  const p = parts(entry);
  const Tag = href ? "a" : "button";
  return (
    <Tag
      href={href}
      type={href ? undefined : "button"}
      data-entry
      className={styles.entry}
      aria-current={selected ? "true" : undefined}
      onClick={(e: MouseEvent) => {
        if (!e.metaKey && !e.ctrlKey && !e.shiftKey && !e.altKey && e.button === 0) onSelect();
      }}
    >
      <span className={styles.top}>
        <Chip tone="agent">{p.chip}</Chip>
        <span className={styles.where}>{p.where}</span>
        <span className={styles.when}>{ago(entry.created_at)}</span>
      </span>
      <span className={styles.title}>{p.title}</span>
      <span className={styles.top}>
        <span className={styles.author_display ?? styles.author}>{p.author}</span>
        <span className={styles.sub}>· {p.sub}</span>
      </span>
    </Tag>
  );
}
