import { DeckArt } from "../../mtg/index.ts";
import { useSession } from "../../app/Session.tsx";
import { useState } from "react";
import { api } from "../../api/client.ts";
import { useVoice } from "../../app/Preferences.tsx";
import { href, navigate } from "../../app/routes.ts";
import { Box, Button, ErrorNote, Stack, Text } from "../../ui/index.ts";
import styles from "./FirstRun.module.css";

/** Three ways in, with no knowledge of agents or deck terminology required. */
export function FirstRun({ onCreate, onImport }: { onCreate: () => void; onImport: () => void }) {
  const voice = useVoice();
  const { session } = useSession();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function sample() {
    if (busy) return;
    setBusy(true); setError(null);
    try { const { state } = await api.createSample(); navigate({ page: "deck", deckId: state.deck.id }); }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); setBusy(false); }
  }
  return <DeckArt cards={session.backdrop ? [session.backdrop] : null} variant="backdrop" priority="auto"><section aria-label="Start your first deck" className={styles.firstRun}>
    <Stack gap={4}><h2>Your first deck starts here</h2><Text as="p" tone="muted">{voice("Build a Commander deck, bring a list you already have, or explore a sample.", "Build a deck yourself, bring a list, or ask your connected agent to help.")}</Text></Stack>
    <div className={styles.choices}>
      <Box><Stack gap={5}><h3>Start a new deck</h3><Text as="p" tone="muted">{voice("Start with a name. Add a commander and cards as your plan takes shape.", "Start with a name, then build the list yourself or with your agent.")}</Text><Button variant="primary" onClick={onCreate} aria-haspopup="dialog">New deck</Button></Stack></Box>
      <Box><Stack gap={5}><h3>Import a deck</h3><Text as="p" tone="muted">{voice("Paste an Archidekt or Moxfield text export. Review the changes before applying them.", "Bring an Archidekt or Moxfield list, then review it with your agent.")}</Text><Button onClick={onImport}>Import a deck</Button></Stack></Box>
      <Box><Stack gap={5}><h3>Try the sample deck</h3><Text as="p" tone="muted">{voice("Explore a filled list, its plan, and example decisions. It is a private copy you can change.", "See a sample list, brief, memories and proposals, with every example agent write attributed.")}</Text><Button disabled={busy} onClick={() => void sample()}>{busy ? "Creating sample…" : "Try the sample deck"}</Button></Stack></Box>
    </div>
    {error && <ErrorNote>{error}</ErrorNote>}
    <Text as="p" size="sm" tone="muted">{voice("Want help from an AI you already use?", "Manage your agent connections in")} <a className={styles.link} href={href({ page: "settings" })}>Settings → Connect an agent</a>.</Text>
  </section></DeckArt>;
}
