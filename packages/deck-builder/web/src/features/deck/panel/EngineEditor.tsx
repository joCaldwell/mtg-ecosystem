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
  const [results, setResults] = useState<CardData[] | null>(null);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // Debounced and abortable: only the latest query's answer is ever shown.
  useEffect(() => {
    const q = query.trim();
    if (!q) {
      setResults(null);
      setSearchError(null);
      return;
    }
    const abort = new AbortController();
    const timer = setTimeout(() => {
      api
        .search(q, deckId, true, abort.signal)
        .then((found) => {
          setResults(found.slice(0, 8));
          setSearchError(null);
        })
        .catch((e: unknown) => {
          if (abort.signal.aborted) return;
          setSearchError(e instanceof Error ? e.message : String(e));
        });
    }, SEARCH_DELAY_MS);
    return () => {
      clearTimeout(timer);
      abort.abort();
    };
  }, [query, deckId]);

  function addPiece(card: CardData) {
    setPieces((list) => (list.some((p) => p.oracle_id === card.oracle_id) ? list : [...list, { oracle_id: card.oracle_id, name: card.name }]));
    setQuery("");
    setResults(null);
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
    if (e.key === "Escape") {
      e.stopPropagation();
      onClose();
    }
  }

  const q = query.trim();
  return (
    <Box tone="neutral">
      <form className={styles.form} onSubmit={submit} onKeyDown={onKeyDown}>
        <Field
          label="Name"
          // The name is the key the server upserts on, so editing through a
          // changed name would leave the original engine behind rather than
          // rename it.
          hint={engine ? "To rename an engine, remove it and add it again." : undefined}
        >
          <TextField value={name} onChange={(e) => setName(e.target.value)} readOnly={!!engine} autoFocus={!engine} />
        </Field>
        <Field label="Description" hint="What the engine does. Markdown, and [[Card Name]] for a card.">
          <TextArea autosize rows={2} value={description} onChange={(e) => setDescription(e.target.value)} autoFocus={!!engine} />
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
          <Button type="submit" variant="primary" disabled={busy || !name.trim()}>
            {busy ? "Saving…" : engine ? "Save engine" : "Add engine"}
          </Button>
        </div>
      </form>
    </Box>
  );
}
