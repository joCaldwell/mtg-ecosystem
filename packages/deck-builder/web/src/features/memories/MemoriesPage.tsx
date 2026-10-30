import { useVoice } from "../../app/Preferences.tsx";
import { useEffect, useState } from "react";
import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { Button, Text, Pill, EmptyState, ErrorNote, Eyebrow, Field, Loading, Page, PageHeader, Select, Stack, TextField } from "../../ui/index.ts";
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
    [scope, showOld, q, cursor, basis], { resources: ["memories", "decks"] },
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
  const showFilters = list.data != null && (filtered || cursors.length > 0 || shown.length > 0);

  return (
    <Page
      header={
        <PageHeader
          title="Memories"
        />
      }
    >
      {list.error && <ErrorNote>{list.error}</ErrorNote>}

      <Stack gap={4}>
        {!(addToDeck != null && archived.has(addToDeck)) && <AddMemory
          key={addToDeck ?? "global"}
          scope={addToDeck != null ? "deck" : "global"}
          deckId={addToDeck ?? undefined}
          scopeLabel={`Adding to: ${addToDeck != null ? decks.get(addToDeck) : "Global"}`}
          onAdded={() => {
            list.reload();
          }}
        />}
        {addToDeck != null && archived.has(addToDeck) && <Text tone="muted" size="sm">Archived deck memories are read-only.</Text>}
      </Stack>

      {/* Filters only once there is something to filter: with no memories at all
          they would only explain an empty list. */}
      {showFilters && <div className={styles.filters}>
        <Field label="Scope" className={styles.scope}>
          <Select value={scope} onChange={event => {
            const value = event.target.value;
            setScope(value === "all" || value === "global" ? value : Number(value));
          }}>
            <option value="all">All scopes</option>
            <option value="global">Global</option>
            {scopes.map(deck => <option key={deck.id} value={deck.id}>{deck.name}{deck.category === "archived" ? " · Archived" : ""}</option>)}
          </Select>
        </Field>
        <div className={styles.group} role="group" aria-label="Basis">
          <Text size="sm" tone="muted">Basis</Text>
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
      </div>}

      {list.loading ? (
        <Loading />
      ) : (
        <Stack gap={4} className={styles.listSection}>
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

      {(cursors.length > 0 || list.data?.has_more) && (
        <nav aria-label="Memory pages">
          <Button disabled={cursors.length === 0} onClick={() => setCursors(cursors.slice(0, -1))}>Previous 100</Button>{" "}
          <Text>Page {cursors.length + 1}</Text>{" "}
          <Button disabled={!list.data?.has_more} onClick={() => {
            const last = shown[shown.length - 1];
            setCursors([...cursors, { before_id: last.id, before_revised_at: last.revised_at }]);
          }}>Next 100</Button>
        </nav>
      )}
    </Page>
  );
}
