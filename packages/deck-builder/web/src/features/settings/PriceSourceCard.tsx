import { useEffect, useState } from "react";
import { api, type PriceSource, type Settings } from "../../api/client.ts";
import { Box, Button, ErrorNote, Field, Row, Select, Stack, Text } from "../../ui/index.ts";

/** Personal prices, below both front doors; agents can only read (§4.4). */
export function PriceSourceCard({ source, onSaved }: { source: Settings["price_source"]; onSaved: () => void }) {
  const [value, setValue] = useState(source);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => setValue(source), [source.currency, source.store]);
  const dirty = value.currency !== source.currency || value.store !== source.store;
  async function save() {
    setBusy(true); setError(null);
    try { await api.updateSettings({ price_source: value }); onSaved(); }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(false); }
  }
  return <Box><Stack gap={5}>
    <Text as="h2" size="lg" weight="bold">Price source</Text>
    <Text tone="muted" size="sm">Daily Scryfall estimates of the cheapest playable printing; no currency conversion.</Text>
    <Row gap={5} wrap>
      <Field label="Currency"><Select options={[{ value: "USD", label: "USD — US dollars" }, { value: "EUR", label: "EUR — euros" }]} value={value.currency} onChange={currency => setValue({ currency: currency as PriceSource["currency"], store: "lowest" })} /></Field>
      <Field label="Store"><Select options={[{ value: "lowest", label: `Lowest in ${value.currency} (${value.currency === "USD" ? "TCGplayer" : "Cardmarket"})` }, value.currency === "USD" ? { value: "tcgplayer", label: "TCGplayer" } : { value: "cardmarket", label: "Cardmarket" }]} value={value.store} onChange={store => setValue({ ...value, store: store as PriceSource["store"] })} /></Field>
    </Row>
    {error && <ErrorNote>{error}</ErrorNote>}
    <Row><Button variant="primary" disabled={busy || !dirty} onClick={save}>{busy ? "Saving…" : "Save price source"}</Button></Row>
  </Stack></Box>;
}
