import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { api, type DeckCard } from "../../../api/client.ts";
import { Stepper } from "../../../ui/index.ts";
import { useDeck } from "../DeckStore.tsx";
import styles from "./CopiesControl.module.css";

// A quantity write is one entry in the decisions log and one undo, so a run of
// clicks waits this long, showing its running number, and saves as one write.
const SETTLE_MS = 500;

/** The copies of a card in the deck. Mount per card (`key`), so a run of clicks
 *  saves to the card it was made on, and is saved if the panel moves on. */
export function CopiesControl({ card }: { card: DeckCard }) {
  const { deckId, run } = useDeck();
  const [draft, setDraft] = useState(String(card.quantity));
  const latest = useRef({ draft, quantity: card.quantity });
  const settling = useRef<ReturnType<typeof setTimeout> | null>(null);
  const writing = useRef(false);
  const queued = useRef<number | null>(null);
  useLayoutEffect(() => {
    latest.current.quantity = card.quantity;
  });

  function show(text: string) {
    latest.current.draft = text;
    setDraft(text);
  }

  function holdFire() {
    if (settling.current !== null) clearTimeout(settling.current);
    settling.current = null;
  }

  // A live refresh mid-run must not overwrite the number being built.
  useEffect(() => {
    if (settling.current === null && !writing.current) show(String(card.quantity));
  }, [card.quantity]);

  const parsed = (text: string) => {
    const n = Number(text.trim());
    return /^\d+$/.test(text.trim()) && n >= 1 ? n : null;
  };

  // One at a time, newest wins: each write is the absolute number, and a later
  // one sent early could land first and leave the older number saved.
  async function write(quantity: number) {
    queued.current = quantity;
    if (writing.current) return;
    writing.current = true;
    let saved = true;
    while (saved && queued.current !== null) {
      const next = queued.current;
      queued.current = null;
      saved = (await run(() => api.updateCard(deckId, card.oracle_id, { quantity: next }))) !== null;
    }
    writing.current = false;
    queued.current = null;
    if (!saved) show(String(latest.current.quantity));
  }

  function commit() {
    holdFire();
    const n = parsed(latest.current.draft);
    if (n === null) return show(String(latest.current.quantity));
    show(String(n));
    if (n !== latest.current.quantity || writing.current) void write(n);
  }

  function settle() {
    holdFire();
    settling.current = setTimeout(commit, SETTLE_MS);
  }

  const current = parsed(draft) ?? card.quantity;
  const atFloor = current <= 1;
  const atLimit = card.copy_limit !== null && current >= card.copy_limit;

  function step(delta: 1 | -1) {
    show(String((parsed(latest.current.draft) ?? latest.current.quantity) + delta));
    settle();
  }

  // Leaving with a run unsaved (another card, a closed panel) still saves it.
  useEffect(() => () => {
    if (settling.current !== null) commit();
  }, []);

  return (
    <Stepper
      className={styles.control}
      value={draft}
      aria-label={`Copies of ${card.name}`}
      upLabel={`Add a copy of ${card.name}`}
      downLabel={`Remove a copy of ${card.name}`}
      upDisabled={atLimit}
      downDisabled={atFloor}
      onStepUp={() => step(1)}
      onStepDown={() => step(-1)}
      onChange={(e) => {
        holdFire();
        show(e.target.value);
      }}
      onFocus={(e) => e.currentTarget.select()}
      onBlur={(e) => {
        // Moving onto an arrow continues the same edit, so it is one write.
        if (e.currentTarget.parentElement?.contains(e.relatedTarget)) settle();
        else commit();
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") e.currentTarget.blur();
        if (e.key === "Escape") {
          e.stopPropagation();
          holdFire();
          show(String(card.quantity));
        }
      }}
    />
  );
}
