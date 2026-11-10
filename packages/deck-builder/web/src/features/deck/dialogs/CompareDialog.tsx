import { useState } from "react";
import { api, type DeckComparison } from "../../../api/client.ts";
import { useQuery } from "../../../api/useQuery.ts";
import { Button, Dialog, ErrorNote, Field, Loading, Row, Select, Spacer, Stack, Text } from "../../../ui/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import { Differences } from "./Differences.tsx";
import { useAttempt } from "./useAttempt.ts";

// Compare this deck with any other, and record which deck it was forked
// from — a deck copied by hand has no such link until it is set here.
export function CompareDialog({ onClose }: { onClose: () => void }) {
  const { deckId } = useDeck();
  const { deck } = useDeckState();
  const parent = deck.parent;
  // A fork of another user's shared deck compares with it while it can still be read (§18.4).
  const upstream = deck.upstream;
  const [other, setOther] = useState<number | "upstream" | null>(parent?.id ?? (upstream ? "upstream" : null));
  const { error, mutate } = useAttempt();

  const decks = useQuery(() => api.listDecks(), [], "decks");
  const others = (decks.data ?? []).filter((d) => d.id !== deckId);

  // Refetched when this deck's revision moves, so the diff never describes a list that has changed.
  const comparison = useQuery(
    () => (other == null ? Promise.resolve(null) : other === "upstream" ? api.compareShared(deckId, upstream!.slug) : api.compareDecks(deckId, other)),
    [deckId, other, deck.revision], "deck",
  );
  // The previous result stays while a refetch runs; it must not be shown for a different deck.
  const diff = comparison.data && (other === "upstream" ? comparison.data.b.slug === upstream?.slug : comparison.data.b.id === other) ? comparison.data : null;

  return (
    <Dialog title="Compare decks" width="lg" onClose={onClose} footer={<Button onClick={onClose}>Close</Button>}>
      <Stack gap={6}>
        <Row align="end" wrap>
          <Field label="Compare with">
            <Select
              options={[
                { value: "", label: "Choose a deck…" },
                ...(upstream ? [{ value: "upstream", label: `${upstream.owner_handle}/${upstream.name} (forked from, shared)` }] : []),
                ...others.map((d) => ({ value: String(d.id), label: `${d.name}${d.id === parent?.id ? " (parent)" : ""}` })),
              ]}
              value={String(other ?? "")}
              onChange={(value) => setOther(value === "" ? null : value === "upstream" ? "upstream" : Number(value))}
              data-autofocus=""
            />
          </Field>
          <Spacer />
          {typeof other === "number" && other !== parent?.id ? (
            <Button
              size="sm"
              title="Record that this deck was forked from the one selected"
              onClick={() => typeof other === "number" && mutate(() => api.setDeckParent(deckId, other))}
            >
              Mark this deck as a fork of it
            </Button>
          ) : null}
          {parent ? (
            <Button size="sm" onClick={() => mutate(() => api.setDeckParent(deckId, null))}>
              Unlink from {parent.name}
            </Button>
          ) : null}
        </Row>

        {error ? <ErrorNote>{error}</ErrorNote> : null}
        {comparison.error ? <ErrorNote>{comparison.error}</ErrorNote> : null}
        {decks.data && others.length === 0 ? (
          <Text tone="muted" as="p">
            There are no other decks to compare with. Make or fork another deck first.
          </Text>
        ) : null}
        {other != null && !diff && !comparison.error ? <Loading /> : null}
        {diff ? <Differences diff={diff} /> : null}
      </Stack>
    </Dialog>
  );
}
