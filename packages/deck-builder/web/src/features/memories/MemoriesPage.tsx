import { useEffect, useState } from "react";
import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { Box, EmptyState, ErrorNote, Eyebrow, Loading, Meter, Page, PageHeader, Section, Stack, TextField } from "../../ui/index.ts";
import { AddMemory } from "./AddMemory.tsx";
import { MemoryCard } from "./MemoryCard.tsx";
import styles from "./MemoriesPage.module.css";

type ScopeFilter = "all" | "global" | number;
type BasisFilter = "any" | "stated" | "inferred";

function Pill({ on, onClick, children }: { on: boolean; onClick: () => void; children: string }) {
  return (
    <button type="button" aria-pressed={on} className={`${styles.pill} ${on ? styles.pillOn : ""}`} onClick={onClick}>
      {children}
    </button>
  );
}

/** Every memory (spec §7.4, §7.5): browse by scope, basis and status, search, correct, retire. */
export function MemoriesPage() {
  const [scope, setScope] = useState<ScopeFilter>("all");
  const [basis, setBasis] = useState<BasisFilter>("any");
  const [showOld, setShowOld] = useState(false);
  const [search, setSearch] = useState("");
  const [q, setQ] = useState("");

  // The search box feeds the query after a pause, so typing is not a request per key.
  useEffect(() => {
    const t = setTimeout(() => setQ(search.trim()), 250);
    return () => clearTimeout(t);
  }, [search]);

  // The route's `status` defaults to "active"; "any" returns everything.
  const list = useQuery(
    () =>
      api.listMemories({
        scope: typeof scope === "number" ? "deck" : scope === "global" ? "global" : undefined,
        deck: typeof scope === "number" ? scope : undefined,
        status: showOld ? "any" : "active",
        q,
      }),
    [scope, showOld, q],
  );
  // Unfiltered, so the scope pills and the budgets stay put while the list narrows.
  const overview = useQuery(() => api.listMemories({ status: "any" }), []);

  const decks = new Map<number, string>();
  for (const m of overview.data?.memories ?? [])
    if (m.deck_id != null && m.deck_name) decks.set(m.deck_id, m.deck_name);

  // If the selected deck's last memory disappears the pill goes with it; fall back to all.
  useEffect(() => {
    if (typeof scope === "number" && overview.data && !decks.has(scope)) setScope("all");
  });

  const shown = (list.data?.memories ?? []).filter((m) => basis === "any" || m.basis === basis);
  const g = overview.data?.global_budget;
  const budgets = overview.data?.budgets;
  const filtered = scope !== "all" || basis !== "any" || showOld || q !== "";
  const addToDeck = typeof scope === "number" ? scope : null;

  return (
    <Page
      header={
        <PageHeader
          title="Memories"
          subtitle="What agents have learned about you. They write these without asking; you can read, correct or retire every one."
        />
      }
    >
      {(overview.error || list.error) && <ErrorNote>{overview.error ?? list.error}</ErrorNote>}

      {g && budgets && (
        <Section title="What loads into an agent's context, in estimated tokens">
          <div className={styles.budgets}>
            <Box padding="sm">
              <div className={styles.budgetHead}>
                <h3 className={styles.budgetTitle}>Global</h3>
                <span className={styles.grow} />
                <span className={styles.num}>
                  {g.used.toLocaleString()} of {g.budget.toLocaleString()}
                </span>
              </div>
              <Meter value={g.used} max={g.budget} tone={g.omitted > 0 ? "warn" : "agent"} aria-label="Global memory budget used" />
              <p className={styles.small}>
                {g.omitted > 0
                  ? `${g.omitted} did not fit and are found by search only. Ask an agent to consolidate.`
                  : "Everything global loads at the start of a session."}
              </p>
            </Box>
            <Box padding="sm">
              <div className={styles.budgetHead}>
                <h3 className={styles.budgetTitle}>Per deck</h3>
                <span className={styles.grow} />
                <span className={styles.num}>{budgets.deck.toLocaleString()} each</span>
              </div>
              <p className={styles.small}>
                Each deck loads its own memories up to {budgets.deck.toLocaleString()} estimated tokens. A deck's meter
                is on that deck's Memories tab.
              </p>
            </Box>
          </div>
        </Section>
      )}

      <Stack gap={4}>
        <div className={styles.filters}>
          <div className={styles.group} role="group" aria-label="Scope">
            <Pill on={scope === "all"} onClick={() => setScope("all")}>All scopes</Pill>
            <Pill on={scope === "global"} onClick={() => setScope("global")}>Global</Pill>
            {[...decks].map(([id, name]) => (
              <Pill key={id} on={scope === id} onClick={() => setScope(id)}>
                {name}
              </Pill>
            ))}
          </div>
          <div className={styles.group} role="group" aria-label="Basis">
            <Pill on={basis === "any"} onClick={() => setBasis("any")}>Any basis</Pill>
            <Pill on={basis === "stated"} onClick={() => setBasis("stated")}>Stated</Pill>
            <Pill on={basis === "inferred"} onClick={() => setBasis("inferred")}>Inferred</Pill>
          </div>
          <div className={styles.group}>
            <Pill on={showOld} onClick={() => setShowOld(!showOld)}>Show retired and superseded</Pill>
          </div>
          <TextField
            type="search"
            className={styles.search}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search memory text or a card"
            aria-label="Search memories"
          />
        </div>

        <AddMemory
          key={addToDeck ?? "global"}
          scope={addToDeck != null ? "deck" : "global"}
          deckId={addToDeck ?? undefined}
          onAdded={() => {
            list.reload();
            overview.reload();
          }}
        />
        <span className={styles.addTo}>
          {addToDeck != null
            ? `Adding to ${decks.get(addToDeck)}, because that deck is selected above.`
            : "Adding a global memory. Select a deck above to add one to that deck instead."}
        </span>
      </Stack>

      {list.loading ? (
        <Loading />
      ) : (
        <Stack gap={4}>
          <div className={styles.listHead}>
            <Eyebrow>{shown.length === 1 ? "1 memory" : `${shown.length} memories`}</Eyebrow>
          </div>
          {shown.length === 0 ? (
            <EmptyState title={filtered ? "No memories match these filters." : "No memories yet."}>
              {filtered
                ? "Widen the scope or basis, include retired ones, or clear the search."
                : "Agents write one when they learn something about how you build. You can write one above."}
            </EmptyState>
          ) : (
            <div className={styles.list}>
              {shown.map((m) => (
                <MemoryCard
                  key={m.id}
                  memory={m}
                  showScope={scope === "all"}
                  onChanged={() => {
                    list.reload();
                    overview.reload();
                  }}
                />
              ))}
            </div>
          )}
        </Stack>
      )}

      <p className={styles.note}>
        <b>Stated</b> means you said it, in a ruling reason or directly. <b>Inferred</b> is an agent's guess, shown
        dashed until you confirm it. An agent can never edit or retire a memory you wrote.
      </p>
    </Page>
  );
}
