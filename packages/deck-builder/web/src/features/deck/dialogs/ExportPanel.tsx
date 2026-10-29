import { useState } from "react";
import { api } from "../../../api/client.ts";
import { useQuery } from "../../../api/useQuery.ts";
import { Button, Checkbox, ErrorNote, Row, Spacer, Stack, Text, TextArea, useToast } from "../../../ui/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import { useSharedDeckView } from "../DeckViewContext.ts";
import styles from "./ExportPanel.module.css";

export function ExportPanel() {
  const { deckId } = useDeck();
  const { deck, cards } = useDeckState();
  const { matches } = useSharedDeckView();
  const { toast } = useToast();
  const [filtered, setFiltered] = useState(matches.active);
  const [categories, setCategories] = useState(!matches.active);
  // When every match is on the maybe list, a filtered export without it would
  // be empty, so it opens with the maybe list included.
  const [includeMaybe, setIncludeMaybe] = useState(() => matches.active && matches.addIds.size === 0 && matches.cardIds.size > 0
    && [...matches.cardIds].every(id => cards.some(c => c.oracle_id === id && c.maybe)));
  const [copyError, setCopyError] = useState<string | null>(null);

  const oracleIds = filtered ? [...matches.cardIds].sort() : undefined;
  const queryKey = JSON.stringify([deckId, categories, includeMaybe, oracleIds, deck.revision]);
  // Keep a previous scope's response from being copied while the new export loads.
  const exported = useQuery(
    async () => ({ key: queryKey, result: await api.exportDeck(deckId, { categories, oracleIds, includeMaybe }) }),
    [queryKey],
  );
  const result = exported.data?.key === queryKey && !exported.error ? exported.data.result : null;
  // An empty filtered export whose matches sit only on the maybe list says so;
  // the Include maybe list box above is the way to include them.
  const maybeMatches = filtered && !includeMaybe
    ? cards.filter((c) => c.maybe && matches.cardIds.has(c.oracle_id)).length
    : 0;

  async function copy() {
    if (!result?.text) return;
    setCopyError(null);
    try {
      await navigator.clipboard.writeText(result.text);
      toast(`Copied ${result.line_count} ${result.line_count === 1 ? "line" : "lines"}.`);
    } catch {
      setCopyError("The browser blocked the clipboard. Select the text and copy it by hand.");
    }
  }

  return (
    <Stack gap={5}>
      <Row gap={6} wrap>
        <Checkbox checked={filtered} onChange={setFiltered} disabled={!matches.active && !filtered} label="Only current filter matches" />
        <Checkbox checked={includeMaybe} onChange={setIncludeMaybe} label="Include maybe list" />
        <Checkbox checked={categories} onChange={setCategories} label="Include categories" />
      </Row>
      {exported.error ? <ErrorNote>{exported.error}</ErrorNote> : null}
      {copyError ? <ErrorNote>{copyError}</ErrorNote> : null}

      {result && result.text ? (
        <TextArea
          readOnly
          rows={12}
          value={result.text}
          aria-label="Exported list"
          className={styles.text}
          onFocus={(e) => e.currentTarget.select()}
        />
      ) : result && maybeMatches > 0 ? (
        <Text tone="muted" as="p" className={styles.empty}>
          {maybeMatches === 1 ? "1 match is on the maybe list." : `${maybeMatches} matches are on the maybe list.`}
        </Text>
      ) : result ? (
        <Text tone="muted" as="p" className={styles.empty}>
          {filtered ? "No cards match these filters and export options." : "Nothing to export with these options."}
        </Text>
      ) : !exported.error ? <Text tone="muted">Preparing export…</Text> : null}

      <Row>
        {result ? (
          <Text tone="muted" size="sm">
            <Text mono tone="soft">
              {result.card_count}
            </Text>{" "}
            {result.card_count === 1 ? "card" : "cards"} on{" "}
            <Text mono tone="soft">
              {result.line_count}
            </Text>{" "}
            {result.line_count === 1 ? "line" : "lines"}
            {" · Archidekt format · pending proposals excluded"}
          </Text>
        ) : null}
        <Spacer />
        {/* Disabled looks disabled: an empty export has no primary action. */}
        <Button variant={result?.text ? "primary" : "default"} onClick={copy} disabled={!result?.text}>
          Copy
        </Button>
      </Row>
    </Stack>
  );
}
