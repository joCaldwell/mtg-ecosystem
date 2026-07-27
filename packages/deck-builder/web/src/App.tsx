import { useEffect, useState } from "react";
import { CardPeekProvider } from "./CardPeek.tsx";
import { DeckList } from "./DeckList.tsx";
import { DeckView } from "./DeckView.tsx";

function parseHash(): number | null {
  const m = window.location.hash.match(/^#\/deck\/(\d+)$/);
  return m ? Number(m[1]) : null;
}

export function App() {
  const [deckId, setDeckId] = useState<number | null>(parseHash());

  useEffect(() => {
    const onHash = () => setDeckId(parseHash());
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);

  // Above the router: the popover is anchored to the viewport and outlives
  // nothing, but every surface that names a card sits underneath here.
  return (
    <CardPeekProvider>
      {deckId == null ? <DeckList /> : <DeckView deckId={deckId} key={deckId} />}
    </CardPeekProvider>
  );
}
