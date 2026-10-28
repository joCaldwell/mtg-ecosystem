import { useState } from "react";
import { api, type Settings } from "../../api/client.ts";
import { ago } from "../../domain/time.ts";
import { Box, Button, ErrorNote, Row, Spacer, Stack, Text } from "../../ui/index.ts";

/** The card database and its prices, ingested from Scryfall's bulk files (spec §4.1, §4.4). */
export function CardDataCard({ cards, jobs, onChanged, canRefresh = true }: { cards: Settings["cards"]; jobs: Settings["jobs"]; onChanged: () => void; canRefresh?: boolean }) {
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
        `Refresh ${next.jobs?.active === "refresh" ? "started" : "queued"}. You can keep using the app while it runs.`,
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
          <Button size="sm" onClick={refresh} disabled={!canRefresh || busy || jobs?.active === "refresh" || jobs?.queued.includes("refresh")}>
            {busy ? "Queuing…" : jobs?.active === "refresh" ? "Refreshing…" : "Refresh now"}
          </Button>
          {!canRefresh && <Text size="sm" tone="muted">Admin only</Text>}
        </Row>
        <Text tone="muted" size="sm" as="p">
          {cards.prices.as_of
            ? `${cards.prices.priced.toLocaleString()} priced cards · ${cards.prices.store === "tcgplayer" ? "TCGplayer" : "Cardmarket"} · ${cards.prices.currency} · ${cards.prices.as_of.slice(0,10)} · daily estimates`
            : "No prices loaded yet. Refresh to load them."}
        </Text>
        {jobs && <Stack gap={2}>
          {(["refresh", "backup"] as const).map(name => <Text key={name} tone={jobs[name].result === "error" ? "cut" : "muted"} size="sm">
            {name === "refresh" ? "Card refresh" : "Backup"}: {jobs.active === name ? "running" : jobs[name].result ?? "not run yet"}
            {jobs[name].last_run ? ` · last run ${ago(jobs[name].last_run!)}` : ""}
            {` · next ${new Date(jobs[name].next_run).toLocaleString()}`}
          </Text>)}
        </Stack>}
        {busy && (
          <Text tone="muted" size="sm" role="status">
            Queuing the background refresh.
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
