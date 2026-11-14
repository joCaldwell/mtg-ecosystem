// Adding or changing an engine (spec §6.2). Pieces are cards, so they are found
// by searching rather than typed as names that might not resolve.

import { useEffect, useState, type FormEvent, type KeyboardEvent } from "react";
import { api, type CardData, type Engine } from "../../../api/client.ts";
import { ManaCost } from "../../../mtg/index.ts";
import { Box, Button, ErrorNote, Field, Icon, IconButton, Text, TextArea, TextField } from "../../../ui/index.ts";
import { useDeck } from "../DeckStore.tsx";
import styles from "./EngineEditor.module.css";

interface Piece {
  oracle_id: string;
  name: string;
}

const SEARCH_DELAY_MS = 250;

export function EngineEditor({ engine, onClose }: { engine?: Engine; onClose: () => void }) {
  const { deckId, run } = useDeck();
  const [name, setName] = useState(engine?.name ?? "");
  const [description, setDescription] = useState(engine?.description ?? "");
  const [pieces, setPieces] = useState<Piece[]>(engine?.pieces.map((p) => ({ oracle_id: p.oracle_id, name: p.name })) ?? []);
  const [query, setQuery] = useState("");
  const q = query.trim();
  const searchKey = JSON.stringify([deckId, q]);
  const [response, setResponse] = useState<{ key: string; cards: CardData[]; error: string | null } | null>(null);
  const current = !!q && response?.key === searchKey;
  const results = current ? response.cards : null;
  const searchError = current ? response.error : null;
  const [busy, setBusy] = useState(false);

  // Debounced and abortable: only the latest query's answer is ever shown.
  useEffect(() => {
    if (!q) return;
    const abort = new AbortController();
    const timer = setTimeout(() => {
      api
        .search(q, deckId, true, abort.signal)
        .then((found) => {
          if (!abort.signal.aborted) setResponse({ key: searchKey, cards: found.slice(0, 8), error: null });
        })
        .catch((e: unknown) => {
          if (abort.signal.aborted) return;
          setResponse({ key: searchKey, cards: [], error: e instanceof Error ? e.message : String(e) });
        });
    }, SEARCH_DELAY_MS);
    return () => {
      clearTimeout(timer);
      abort.abort();
    };
  }, [q, deckId]);

  function addPiece(card: CardData) {
    setPieces((list) => (list.some((p) => p.oracle_id === card.oracle_id) ? list : [...list, { oracle_id: card.oracle_id, name: card.name }]));
    setQuery("");
    setResponse(null);
  }

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (!name.trim() || busy) return;
    setBusy(true);
    const result = await run(() =>
      api.setEngine(deckId, name.trim(), description, pieces.map((p) => ({ oracle_id: p.oracle_id }))),
    );
    setBusy(false);
    if (result) onClose();
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.nativeEvent.isComposing || e.nativeEvent.keyCode === 229) return;
    if (e.key === "Escape") {
      e.stopPropagation();
      onClose();
    }
  }

  return (
    <Box tone="neutral">
      <form method="post" className={styles.form} onSubmit={submit} onKeyDown={onKeyDown}>
        <Field
          label="Name"
          // The name is the key the server upserts on, so editing through a
          // changed name would leave the original engine behind rather than
          // rename it.
          hint={engine ? "To rename an engine, remove it and add it again." : undefined}
        >
          <TextField name="name" required pattern={".*\\S.*"} disabled={busy} value={name} onChange={(e) => setName(e.target.value)} readOnly={!!engine} autoFocus={!engine} />
        </Field>
        <Field label="Description" hint="What the engine does. Markdown, and [[Card Name]] for a card.">
          <TextArea name="description" disabled={busy} autosize rows={2} value={description} onChange={(e) => setDescription(e.target.value)} autoFocus={!!engine} />
        </Field>

        <div className={styles.pieces}>
          <Text tone="soft" size="sm">
            Pieces · {pieces.length}
          </Text>
          {pieces.map((p) => (
            <div key={p.oracle_id} className={styles.piece}>
              <span className={styles.pieceName}>{p.name}</span>
              <IconButton
                size="sm"
                label={`Remove ${p.name} from this engine`}
                onClick={() => setPieces((list) => list.filter((x) => x.oracle_id !== p.oracle_id))}
              >
                <Icon name="close" size={12} />
              </IconButton>
            </div>
          ))}
          <TextField
            type="search" autoComplete="off" autoCapitalize="none" autoCorrect="off" spellCheck={false} disabled={busy}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search for a card to add"
            aria-label="Search for a card to add as a piece"
          />
          {searchError && <ErrorNote>{searchError}</ErrorNote>}
          {results && results.length === 0 && !searchError && (
            <Text tone="muted" size="sm">
              No card matches “{q}”.
            </Text>
          )}
          {results && results.length > 0 && (
            <ul className={styles.results}>
              {results.map((c) => (
                <li key={c.oracle_id}>
                  <button type="button" className={styles.result} onClick={() => addPiece(c)}>
                    <span className={styles.pieceName}>{c.name}</span>
                    <ManaCost cost={c.mana_cost} />
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div className={styles.actions}>
          <Button onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button type="submit" variant="primary" disabled={busy}>
            {busy ? "Saving…" : engine ? "Save engine" : "Add engine"}
          </Button>
        </div>
      </form>
    </Box>
  );
}
