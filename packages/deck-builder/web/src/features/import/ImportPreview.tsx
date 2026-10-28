import { useId, useLayoutEffect, useRef, useState, type MouseEvent, type ReactNode } from "react";
import type { ImportDiff } from "../../api/client.ts";
import { useCardHover } from "../../mtg/index.ts";
import { Box, ErrorNote, Row, Stack, Text } from "../../ui/index.ts";
import { CardLine } from "./CardLine.tsx";
import styles from "./ImportPreview.module.css";

// The preview is the approval step (spec §12). Problem lines lead the changes
// so omissions can be checked first, followed by cuts in the cut colour.
// Names that did not resolve are never guessed.
export function ImportPreview({ diff }: { diff: ImportDiff }) {
  const hover = useCardHover();
  const warningsId = useId();
  const sectionId = useId();
  const decreases = diff.quantity_changes.filter((q) => q.to < q.from);
  const increases = diff.quantity_changes.filter((q) => q.to > q.from);
  // A misspelled line leaves its card out of the list, so the card shows up as a cut. Say so, so the
  // owner does not read it as a decision.
  const cutNames = new Set([...diff.cuts.map((c) => c.name), ...decreases.map((q) => q.name)]);
  const problems = diff.unresolved.length + diff.ambiguous.length + diff.unparsed.length;
  const changes = useRef<HTMLDivElement>(null);
  const [more, setMore] = useState(0);
  const [scrollbar, setScrollbar] = useState({ height: 100, top: 0 });
  function jump(event: MouseEvent<HTMLAnchorElement>, section: string) {
    event.preventDefault();
    const el = changes.current;
    const group = document.getElementById(`${sectionId}-${section}`) as HTMLDetailsElement | null;
    if (!el || !group) return;
    group.open = true;
    // Only move the changes pane; the omissions and Apply controls stay put.
    el.scrollBy({ top: group.getBoundingClientRect().top - el.getBoundingClientRect().top });
    group.querySelector("summary")?.focus({ preventScroll: true });
    measureRemaining();
  }
  function countLink(section: string, count: number, label: string, tone?: "add" | "cut") {
    const content = <><Text mono tone={count ? tone : "muted"}>{count}</Text> {label}</>;
    return count ? <a className={styles.sectionLink} href={`#${sectionId}-${section}`} onClick={event => jump(event, section)}>{content}</a> : content;
  }
  function measureRemaining() {
    const el = changes.current;
    if (!el) return;
    const bottom = el.getBoundingClientRect().bottom;
    // Rows inside a closed section are not "below"; its summary carries the count.
    setMore(Array.from(el.querySelectorAll("details[open] li[data-import-change]")).filter(row => row.getBoundingClientRect().bottom > bottom + 1).length);
    const height = Math.max(5, 100 * el.clientHeight / Math.max(1, el.scrollHeight));
    const progress = el.scrollTop / Math.max(1, el.scrollHeight - el.clientHeight);
    setScrollbar({ height, top: progress * (100 - height) });
  }
  useLayoutEffect(() => {
    measureRemaining();
    const observer = new ResizeObserver(measureRemaining);
    if (changes.current) observer.observe(changes.current);
    return () => observer.disconnect();
  }, [diff]);

  // Against an empty deck every card is an add: zero counts say nothing, and
  // the routine add and maybe lists start closed behind their counts.
  const emptyTarget = diff.cut_count === 0 && diff.unchanged === 0 && diff.quantity_changes.length === 0 && diff.role_changes.length === 0;
  const segments: Array<[string, ReactNode]> = [];
  if (!emptyTarget || diff.cut_count) segments.push(["cuts", countLink("cuts", diff.cut_count, "to cut", "cut")]);
  if (!emptyTarget || diff.add_count) segments.push(["adds", countLink("adds", diff.add_count, "to add", "add")]);
  if (!emptyTarget) segments.push(["unchanged", <><Text mono>{diff.unchanged}</Text> unchanged</>]);
  if (diff.role_changes.length)
    segments.push(["roles", countLink("roles", diff.role_changes.length, diff.role_changes.length === 1 ? "role change" : "role changes")]);
  if (diff.set_aside.length) segments.push(["aside", countLink("aside", diff.set_aside.length, "set aside")]);
  if (problems)
    segments.push(["problems", <a className={styles.problemLink} href={`#${warningsId}`}><Text mono>{problems}</Text> {problems === 1 ? "problem line" : "problem lines"}</a>]);
  const headings = [problems ? `${warningsId}-skipped` : null, diff.notes.length ? `${warningsId}-check` : null].filter(Boolean).join(" ");

  return (
    <div className={styles.preview}>
      <Stack gap={3} className={styles.summary} role="status" tabIndex={-1} data-import-summary>
        <Text as="p" tone="muted">
          {segments.map(([key, segment], i) => <span key={key}>{i > 0 ? " · " : null}{segment}</span>)}
        </Text>

        {diff.cut_count > 0 ? (
          <ErrorNote>
            Removes {diff.cut_count} {diff.cut_count === 1 ? "card" : "cards"}, including quantity decreases. You can undo the import.
          </ErrorNote>
        ) : null}

      </Stack>
        {/* One callout for everything that needs the owner's attention: lines
            that were skipped, then what the app cannot verify for itself. */}
        {headings ? (
          <Box className={styles.warnings} tone="warn" id={warningsId} role="region" aria-labelledby={headings} tabIndex={0}>
            {problems > 0 ? (
              <>
                <Row gap={4} align="baseline" wrap>
                  <Text as="h3" id={`${warningsId}-skipped`} size="md" weight="medium" tone="warn">
                    Not imported · <Text mono>{problems}</Text>
                  </Text>
                  <Text tone="muted" size="sm">Skipped. Fix the line and preview again.</Text>
                </Row>
                <ul className={styles.problems}>
                {diff.unresolved.map((u) => (
                  <li key={`u${u.line_no}`}>
                    <Text mono tone="muted">
                      line {u.line_no}
                    </Text>{" "}
                    “{u.name}” did not match a card.
                    {u.suggestions.length > 0 ? (
                      <>
                        {" "}
                        Did you mean{" "}
                        {u.suggestions.map((s, i) => (
                          <span key={s}>
                            {i > 0 ? ", " : ""}
                            <span className={styles.suggestion} {...hover(s)}>
                              {s}
                            </span>
                          </span>
                        ))}
                        ?
                      </>
                    ) : (
                      " No close names were found."
                    )}
                    {u.suggestions.some((s) => cutNames.has(s)) ? (
                      <Text tone="cut">
                        {" "}
                        {u.suggestions.filter((s) => cutNames.has(s)).join(", ")} is in the cut list because of this line.
                      </Text>
                    ) : null}
                  </li>
                ))}
                {diff.ambiguous.map((a) => (
                  <li key={`a${a.line_no}`}>
                    <Text mono tone="muted">
                      line {a.line_no}
                    </Text>{" "}
                    “{a.name}” matched more than one card. Add the one you mean through search.
                    <ul className={styles.candidates}>
                      {a.candidates.map((c) => (
                        <CardLine key={c.oracle_id} oracleId={c.oracle_id} name={c.name} detail={c.oracle_id.slice(0, 8)} />
                      ))}
                    </ul>
                  </li>
                ))}
                {diff.unparsed.map((u) => (
                  <li key={`p${u.line_no}`}>
                    <Text mono tone="muted">
                      line {u.line_no}
                    </Text>{" "}
                    could not be read as a card entry: <Text mono>{u.raw}</Text>
                  </li>
                ))}
                </ul>
              </>
            ) : null}
            {diff.notes.length > 0 ? (
              <div className={styles.check} data-after-skipped={problems > 0 || undefined}>
                <Text as="h3" id={`${warningsId}-check`} size="md" weight="medium" tone="warn">Check after importing</Text>
                {diff.notes.map((note) => (
                  <Text key={note} as="p" size="sm" tone="soft">{note}</Text>
                ))}
              </div>
            ) : null}
          </Box>
        ) : null}

      <div className={styles.scrollFrame} data-more={more > 0 || undefined}>
      <div ref={changes} onScroll={measureRemaining} className={styles.changes} role="region" aria-label="Import changes" tabIndex={0}>
        <Stack gap={5}>
        {diff.cuts.length + decreases.length > 0 ? (
          <Group id={`${sectionId}-cuts`} onToggle={measureRemaining} title="Will be cut" count={diff.cut_count} tone="cut">
            {diff.cuts.map((c) => (
              <CardLine
                key={c.oracle_id}
                oracleId={c.oracle_id}
                name={c.name}
                quantity={c.quantity}
                tone="cut"
                label="cut"
                detail={c.role !== "card" ? c.role : undefined}
              />
            ))}
            {decreases.map((q) => (
              <CardLine
                key={q.oracle_id}
                oracleId={q.oracle_id}
                name={q.name}
                tone="cut"
                label="cut"
                detail={`quantity ${q.from} → ${q.to}`}
              />
            ))}
          </Group>
        ) : null}

        {diff.adds.length + increases.length > 0 ? (
          <Group id={`${sectionId}-adds`} onToggle={measureRemaining} open={!emptyTarget} title="Will be added" count={diff.add_count} tone="add">
            {diff.adds.map((a) => (
              <CardLine
                key={a.oracle_id}
                oracleId={a.oracle_id}
                name={a.name}
                quantity={a.quantity}
                tone="add"
                label="add"
                detail={addDetail(a)}
              />
            ))}
            {increases.map((q) => (
              <CardLine
                key={q.oracle_id}
                oracleId={q.oracle_id}
                name={q.name}
                tone="add"
                label="add"
                detail={`quantity ${q.from} → ${q.to}`}
              />
            ))}
          </Group>
        ) : null}

        {diff.role_changes.length > 0 ? (
          <Group id={`${sectionId}-roles`} onToggle={measureRemaining} title="Will change role" count={diff.role_changes.length}>
            {diff.role_changes.map((r) => (
              <CardLine key={r.oracle_id} oracleId={r.oracle_id} name={r.name} detail={`${r.from} → ${r.to}`} />
            ))}
          </Group>
        ) : null}

        {diff.set_aside.length > 0 ? (
          <Group id={`${sectionId}-aside`} onToggle={measureRemaining} open={!emptyTarget} title="Set aside to the maybe list" count={diff.set_aside.length}>
            <li className={styles.note}>
              Listed under Maybeboard or Sideboard, so shortlisted on the maybe list and not added to the deck.
            </li>
            {diff.set_aside.map((c) => (
              <CardLine key={c.oracle_id} oracleId={c.oracle_id} name={c.name} detail={c.category} />
            ))}
          </Group>
        ) : null}


        </Stack>
      </div>
      {/* Keep the native scrollbar operable underneath a persistent visual track,
          including on systems that hide their scrollbar until it is used. */}
      <div className={styles.scrollbar} aria-hidden="true"><span style={{ height: `${scrollbar.height}%`, top: `${scrollbar.top}%` }} /></div>
      </div>
      {more > 0 && <Text as="p" size="sm" tone="muted" className={styles.continuation}>↓ {more} more {more === 1 ? "change" : "changes"} below · Scroll to review</Text>}
    </div>
  );
}

function addDetail(a: ImportDiff["adds"][number]): ReactNode {
  // The command zone is not part of any slot, so a category says nothing more there.
  if (a.role !== "card") return a.role;
  const bits: string[] = [];
  if (a.slot_name) bits.push(`to ${a.slot_name}`);
  else if (a.category) bits.push(`category “${a.category}” matches no slot`);
  return bits.length ? bits.join(" · ") : undefined;
}

function Group({
  id,
  onToggle,
  open = true,
  title,
  count,
  tone,
  children,
}: {
  id: string;
  onToggle: () => void;
  /** Whether it starts open; afterwards the owner's toggle is left alone. */
  open?: boolean;
  title: string;
  count: number;
  tone?: "add" | "cut";
  children: ReactNode;
}) {
  return (
    <details id={id} open={open} className={styles.group} onToggle={onToggle}>
      <summary>
        <Text weight="medium" size="sm" tone={tone}>{title} · <Text mono>{count}</Text></Text>
      </summary>
      <ul className={styles.list}>{children}</ul>
    </details>
  );
}
