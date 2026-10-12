import { useState } from "react";
import { api } from "../../../api/client.ts";
import { useQuery } from "../../../api/useQuery.ts";
import { Button, Checkbox, ErrorNote, Row, Spacer, Stack, Text, TextArea, useToast } from "../../../ui/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import styles from "./ExportPanel.module.css";

export function ExportPanel() {
  const { deckId } = useDeck();
  const { deck } = useDeckState();
  const { toast } = useToast();
  const [categories, setCategories] = useState(true);
  const [copyError, setCopyError] = useState<string | null>(null);

  // Refetched when the revision moves, so a card added elsewhere is in the text.
  const exported = useQuery(
    () => api.exportDeck(deckId, { categories }),
    [deckId, categories, deck.revision],
  );
  const result = exported.data;

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
      ) : result ? (
        <Text tone="muted" as="p" className={styles.empty}>
          Nothing to export. The deck is empty.
        </Text>
      ) : null}

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
        Quantity and name, one card per line, with categories in brackets: Archidekt's own round-trip syntax. Set
        codes and foil markers are left out on purpose. They break some importers, and this app keys cards on
        oracle id, so nothing is lost. The maybe list is not exported.
      </Text>
    </Stack>
  );
}
