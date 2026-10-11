import type { ReactNode } from "react";
import type { ImportDiff } from "../../../api/client.ts";
import { useCardHover } from "../../../mtg/index.ts";
import { Box, ErrorNote, Stack, Text } from "../../../ui/index.ts";
import { CardLine } from "./CardLine.tsx";
import styles from "./ImportPreview.module.css";

// The preview is the approval step (spec §12). Cuts come first and in the
// cut colour, because they are the only part of an import that takes
// something away; the problems come last, but are counted up here so they
// cannot be missed. Names that did not resolve are never guessed.
export function ImportPreview({ diff }: { diff: ImportDiff }) {
  const hover = useCardHover();
  const decreases = diff.quantity_changes.filter((q) => q.to < q.from);
  const increases = diff.quantity_changes.filter((q) => q.to > q.from);
  // A misspelled line leaves its card out of the list, so the card shows up as a cut. Say so, so the
  // owner does not read it as a decision.
  const cutNames = new Set([...diff.cuts.map((c) => c.name), ...decreases.map((q) => q.name)]);
  const problems = diff.unresolved.length + diff.ambiguous.length + diff.unparsed.length;

  return (
    <Stack gap={6}>
      <Text as="p" tone="muted">
        <Text mono tone={diff.cut_count ? "cut" : "muted"}>
          {diff.cut_count}
        </Text>{" "}
        to cut ·{" "}
        <Text mono tone={diff.add_count ? "add" : "muted"}>
          {diff.add_count}
        </Text>{" "}
        to add · <Text mono>{diff.unchanged}</Text> unchanged
        {diff.role_changes.length ? (
          <>
            {" "}
            · <Text mono>{diff.role_changes.length}</Text> {diff.role_changes.length === 1 ? "role change" : "role changes"}
          </>
        ) : null}
        {diff.set_aside.length ? (
          <>
            {" "}
            · <Text mono>{diff.set_aside.length}</Text> set aside
          </>
        ) : null}
        {problems ? (
          <>
            {" "}
            · <Text mono tone="warn">{problems}</Text> <Text tone="warn">{problems === 1 ? "problem line" : "problem lines"}</Text>
          </>
        ) : null}
      </Text>

      {diff.cut_count > 0 ? (
        <ErrorNote>
          Applying this removes {diff.cut_count} {diff.cut_count === 1 ? "card" : "cards"}, including quantity
          decreases. It is one entry in History, so undoing it puts the deck back exactly as it is now.
        </ErrorNote>
      ) : null}

      {diff.cuts.length + decreases.length > 0 ? (
        <Group title="Will be cut" count={diff.cut_count} tone="cut">
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
        <Group title="Will be added" count={diff.add_count} tone="add">
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
        <Group title="Will change role" count={diff.role_changes.length}>
          {diff.role_changes.map((r) => (
            <CardLine key={r.oracle_id} oracleId={r.oracle_id} name={r.name} detail={`${r.from} → ${r.to}`} />
          ))}
        </Group>
      ) : null}

      {diff.set_aside.length > 0 ? (
        <Group title="Set aside to the maybe list" count={diff.set_aside.length}>
          <li className={styles.note}>
            Listed under Maybeboard or Sideboard, so shortlisted on the maybe list and not added to the deck.
          </li>
          {diff.set_aside.map((c) => (
            <CardLine key={c.oracle_id} oracleId={c.oracle_id} name={c.name} detail={c.category} />
          ))}
        </Group>
      ) : null}

      {problems > 0 ? (
        <Box tone="warn">
          <Text weight="medium" tone="warn">
            Not imported · {problems}
          </Text>
          <Text tone="muted" size="sm" as="p">
            These lines are skipped, and the rest of the list still applies. Names are matched exactly and never
            guessed: fix the line and preview again.
          </Text>
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
        </Box>
      ) : null}
    </Stack>
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
  title,
  count,
  tone,
  children,
}: {
  title: string;
  count: number;
  tone?: "add" | "cut";
  children: ReactNode;
}) {
  return (
    <Stack gap={2}>
      <Text weight="medium" size="sm" as="h3" tone={tone}>
        {title} · <Text mono>{count}</Text>
      </Text>
      <ul className={styles.list}>{children}</ul>
    </Stack>
  );
}
