import { Explainer } from "../../ui/index.ts";
import { useVoice } from "../../app/Preferences.tsx";
import { useEffect, useState } from "react";
import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { Button, Text, Pill, EmptyState, ErrorNote, Eyebrow, Loading, Page, PageHeader, Stack, TextField } from "../../ui/index.ts";
import { AddMemory } from "./AddMemory.tsx";
import { MemoryCard } from "./MemoryCard.tsx";
import styles from "./MemoriesPage.module.css";

type ScopeFilter = "all" | "global" | number;
type BasisFilter = "any" | "stated" | "inferred";

/** Every memory (spec §7.4, §7.5): browse by scope, basis and status, search, correct, retire. */
export function MemoriesPage() {
  const voice = useVoice();
  // Keyset paging (§11.3): each earlier page is remembered by the row it
  // started after, so writes between loads never shift rows.
  const [cursors, setCursors] = useState<{ before_id: number; before_revised_at: string }[]>([]);
  const cursor = cursors.at(-1);
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
        q, ...cursor, basis: basis === "any" ? undefined : basis,
      }),
    [scope, showOld, q, cursor, basis],
  );
  const scopes = list.data?.scopes ?? [];
  const archived = new Set(scopes.filter(d => d.category === "archived").map(d => d.id));
  const decks = new Map(scopes.map(d => [d.id, d.name]));
  const shown = list.data?.memories ?? [];
  useEffect(() => setCursors([]), [scope, showOld, q, basis]);
  useEffect(() => {
    if (typeof scope === "number" && list.data && !list.data.scopes.some(deck => deck.id === scope)) setScope("all");
  }, [scope, list.data?.scopes]);
  const filtered = scope !== "all" || basis !== "any" || showOld || q !== "";
  const addToDeck = typeof scope === "number" ? scope : null;

  return (
    <Page
      header={
        <PageHeader
          title="Memories"
          subtitle={voice("Keep the preferences and lessons you want to carry from one deck to the next.", "What agents have learned about you. They write these without asking; you can read, correct or retire every one.")}
        />
      }
    >
      <Explainer title="Memories" summary={voice("A memory keeps a preference or lesson with its reason.", "Memories keep what your agent learns between sessions.")}>Use global memories for tastes that apply to every deck, or choose a deck for notes about that list. Stated means you said it; inferred means a conclusion you can confirm. You can correct or retire any memory. Agents cannot change one you wrote or edited.</Explainer>
      {list.error && <ErrorNote>{list.error}</ErrorNote>}


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

        {!(addToDeck != null && archived.has(addToDeck)) && <AddMemory
          key={addToDeck ?? "global"}
          scope={addToDeck != null ? "deck" : "global"}
          deckId={addToDeck ?? undefined}
          onAdded={() => {
            list.reload();
          }}
        />}
        <span className={styles.addTo}>
          {addToDeck != null && archived.has(addToDeck) ? "Archived deck memories are read-only." : addToDeck != null
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
                : voice("Write a preference or a lesson above to keep it for next time.", "Agents write one when they learn something about how you build. You can write one above.")}
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
                  }}
                />
              ))}
            </div>
          )}
        </Stack>
      )}

      <nav aria-label="Memory pages"><Button disabled={cursors.length === 0} onClick={() => setCursors(cursors.slice(0, -1))}>Previous 100</Button> <Text>Page {cursors.length + 1}</Text> <Button disabled={!list.data?.has_more} onClick={() => { const last = shown[shown.length - 1]; setCursors([...cursors, { before_id: last.id, before_revised_at: last.revised_at }]); }}>Next 100</Button></nav>
      <p className={styles.note}>{voice("Stated memories record what you said. Inferred memories are conclusions to confirm.", "Stated means you said it, in a decision reason or directly. Inferred is an agent's guess, dashed until confirmed. An agent can never edit or retire a memory you wrote.")}</p>
    </Page>
  );
}
