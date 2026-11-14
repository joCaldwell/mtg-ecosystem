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
  const [query, setQuery] = useState(""),
    [cards, setCards] = useState<CardData[]>([]),
    [error, setError] = useState<string | null>(null),
    [loading, setLoading] = useState(false);
  useEffect(() => {
    if (!query.trim()) {
      setCards([]);
      return;
    }
    const abort = new AbortController();
    const timer = setTimeout(() => {
      setLoading(true);
      setError(null);
      void api
        .search(query, undefined, false, abort.signal)
        .then(setCards)
        .catch((e) => {
          if (!abort.signal.aborted) setError(e.message);
        })
        .finally(() => {
          if (!abort.signal.aborted) setLoading(false);
        });
    }, 250);
    return () => {
      clearTimeout(timer);
      abort.abort();
    };
  }, [query]);
  return (
    <Dialog title="Create a token from card data" onClose={onClose}>
      <p>
        Choose a verified card to represent a manual token copy. You can also
        copy a selected table object from the inspection panel.
      </p>
      <TextField
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
