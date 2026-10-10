import { useState } from "react";
import { api, type Settings } from "../../api/client.ts";
import { ago } from "../../domain/time.ts";
import { Box, Button, ErrorNote, Row, Spacer, Stack, Text } from "../../ui/index.ts";

/** The card database, ingested from Scryfall's bulk file (spec §13). */
export function CardDataCard({ cards, onChanged }: { cards: Settings["cards"]; onChanged: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);

  async function refresh() {
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const next = await api.refreshCards();
      setResult(`Loaded ${next.cards.count.toLocaleString()} cards.`);
      onChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Box>
      <Stack gap={5}>
        <Text as="h2" size="lg" weight="bold">
          Card data
        </Text>
        <Row gap={5} wrap>
          <Text>Scryfall oracle-cards</Text>
          <Text mono tone="muted">
            {cards.count.toLocaleString()} cards
          </Text>
          <Text tone="faint">refreshed {cards.updated_at ? ago(cards.updated_at) : "never"}</Text>
          <Spacer />
          <Button size="sm" onClick={refresh} disabled={busy}>
            {busy ? "Refreshing…" : "Refresh now"}
          </Button>
        </Row>
        {busy && (
          <Text tone="muted" size="sm" role="status">
            Downloading Scryfall's bulk file and loading it. This can take a minute.
          </Text>
        )}
        {result && (
          <Text tone="add" size="sm" role="status">
            {result}
          </Text>
        )}
        {error && <ErrorNote>{error}</ErrorNote>}
      </Stack>
    </Box>
  );
}
