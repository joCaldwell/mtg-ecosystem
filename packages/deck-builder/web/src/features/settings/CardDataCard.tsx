import { useState } from "react";
import { api, type Settings } from "../../api/client.ts";
import { ago } from "../../domain/time.ts";
import { Box, Button, ErrorNote, Row, Spacer, Stack, Text } from "../../ui/index.ts";

/** The card database and its prices, ingested from Scryfall's bulk files (spec §4.1, §4.4). */
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
      setResult(
        `Loaded ${next.cards.count.toLocaleString()} cards and ${next.cards.prices.priced.toLocaleString()} prices.`,
      );
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
          <Text>Scryfall bulk data</Text>
          <Text mono tone="muted">
            {cards.count.toLocaleString()} cards
          </Text>
          <Text tone="faint">refreshed {cards.updated_at ? ago(cards.updated_at) : "never"}</Text>
          <Spacer />
          <Button size="sm" onClick={refresh} disabled={busy}>
            {busy ? "Refreshing…" : "Refresh now"}
          </Button>
        </Row>
        <Text tone="muted" size="sm" as="p">
          {cards.prices.as_of
            ? `${cards.prices.priced.toLocaleString()} cards have a price: the cheapest printing of each, from Scryfall's ` +
              `prices, published ${ago(cards.prices.as_of)}. Scryfall updates them daily, and they are estimates.`
            : "No prices loaded yet. Refresh to load them."}
        </Text>
        {busy && (
          <Text tone="muted" size="sm" role="status">
            Downloading Scryfall's cards and every printing's price, then loading them. This can take a minute.
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
