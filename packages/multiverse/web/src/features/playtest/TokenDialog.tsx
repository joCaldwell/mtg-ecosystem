import { useEffect, useState } from "react";
import { api } from "../../api/client.ts";
import type { CardData } from "../../api/client.ts";
import { Button, Dialog, TextField } from "../../ui/index.ts";
export function TokenDialog({
  onClose,
  onCreate,
}: {
  onClose: () => void;
  onCreate: (oracleId: string) => void;
}) {
  const [query, setQuery] = useState("");
  const q = query.trim();
  const [response, setResponse] = useState<{ query: string; cards: CardData[]; error: string | null } | null>(null);
  const current = !!q && response?.query === q;
  const cards = current ? response.cards : [];
  const error = current ? response.error : null;
  const loading = !!q && !current;
  useEffect(() => {
    if (!q) return;
    const abort = new AbortController();
    const timer = setTimeout(() => {
      void api.search(q, undefined, false, abort.signal).then(
        cards => { if (!abort.signal.aborted) setResponse({ query: q, cards, error: null }); },
        error => { if (!abort.signal.aborted) setResponse({ query: q, cards: [], error: error instanceof Error ? error.message : String(error) }); },
      );
    }, 250);
    return () => {
      clearTimeout(timer);
      abort.abort();
    };
  }, [q]);
  return (
    <Dialog title="Create a token from card data" onClose={onClose}>
      <p>
        Choose a verified card to represent a manual token copy. You can also
        copy a selected table object from the inspection panel.
      </p>
      <TextField
        type="search" autoComplete="off" autoCapitalize="none" autoCorrect="off" spellCheck={false}
        data-autofocus
        aria-label="Find card for token"
        value={query}
        placeholder="Search card names or Scryfall syntax"
        onChange={(e) => setQuery(e.target.value)}
      />
      {loading && <p>Searching…</p>}
      {error && <p role="alert">{error}</p>}
      {cards.map((card) => (
        <p key={card.oracle_id}>
          <Button onClick={() => onCreate(card.oracle_id)}>
            Create copy of {card.name}
          </Button>
        </p>
      ))}
    </Dialog>
  );
}
