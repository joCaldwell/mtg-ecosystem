import { useState } from "react";
import { api } from "../../../api/client.ts";
import { useQuery } from "../../../api/useQuery.ts";
import { Button, Checkbox, ErrorNote, Row, Spacer, Stack, Text, TextArea, useToast } from "../../../ui/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import { useSharedDeckView } from "../DeckViewContext.ts";
import styles from "./ExportPanel.module.css";

export function ExportPanel() {
  const { deckId } = useDeck();
  const { deck } = useDeckState();
  const { matches } = useSharedDeckView();
  const { toast } = useToast();
  const [filtered, setFiltered] = useState(matches.active);
  const [categories, setCategories] = useState(!matches.active);
  const [includeMaybe, setIncludeMaybe] = useState(false);
  const [copyError, setCopyError] = useState<string | null>(null);

  const oracleIds = filtered ? [...matches.cardIds].sort() : undefined;
  const queryKey = JSON.stringify([deckId, categories, includeMaybe, oracleIds, deck.revision]);
  // Keep a previous scope's response from being copied while the new export loads.
  const exported = useQuery(
    async () => ({ key: queryKey, result: await api.exportDeck(deckId, { categories, oracleIds, includeMaybe }) }),
    [queryKey],
  );
  const result = exported.data?.key === queryKey && !exported.error ? exported.data.result : null;

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
    <Stack gap={5} className={styles.panel}>
      <Row gap={6} wrap>
        <Checkbox checked={filtered} onChange={setFiltered} disabled={!matches.active && !filtered} label="Only current filter matches" />
        <Checkbox checked={includeMaybe} onChange={setIncludeMaybe} label="Include maybe list" />
        <Checkbox checked={categories} onChange={setCategories} label="Include categories" />
      </Row>
      <Text tone="muted" size="sm">
        Pending proposals are excluded.
      </Text>

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
            .
          </Text>
        ) : null}
        <Spacer />
        <Button variant="primary" onClick={copy} disabled={!result?.text}>
          Copy
        </Button>
      </Row>

      <Text tone="muted" size="sm" as="p">
        Quantity and name per line. Categories use Archidekt syntax; maybe cards use Maybeboard when categories are included.
      </Text>
    </Stack>
  );
}
