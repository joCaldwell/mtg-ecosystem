// Add a card, or search for one (spec §4). One input does both: a name that
// resolves exactly is added; anything else is a query, and its results can be
// added from the list. Nothing is guessed; "No card matches" is the answer
// to a name that is not a card.

import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { api, type CardData } from "../../api/client.ts";
import { annotateResults, type AnnotatedResult } from "../../domain/search.ts";
import { ColorIdentity } from "../../mtg/index.ts";
import { Checkbox, ErrorNote, Icon, Kbd, Text, useHotkeys, useToast } from "../../ui/index.ts";
import { AddBarResult } from "./AddBarResult.tsx";
import { useDeck, useDeckState } from "./DeckStore.tsx";
import { useDeckUi } from "./DeckUi.tsx";
import { useSharedDeckView } from "./DeckViewContext.ts";
import styles from "./AddBar.module.css";

const DEBOUNCE_MS = 150;
const MIN_QUERY = 2;

export function AddBar() {
  const state = useDeckState();
  const { deckId, run } = useDeck();
  const ui = useDeckUi();
  const { marks } = useSharedDeckView();
  const { toast } = useToast();

  const [query, setQuery] = useState("");
  const [everyColour, setEveryColour] = useState(false);
  const [results, setResults] = useState<CardData[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const [index, setIndex] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const box = useRef<HTMLDivElement>(null);
  const adding = useRef(false);
  const listId = useId();

  const q = query.trim();

  // Search as the owner types. Each keystroke supersedes the last: its timer
  // is cleared and its request aborted, so a slow answer to "dra" can never
  // overwrite the answer to "dragon".
  useEffect(() => {
    if (q.length < MIN_QUERY) {
      setResults(null);
      setError(null);
      return;
    }
    const controller = new AbortController();
    const timer = window.setTimeout(() => {
      api
        .search(q, deckId, !everyColour, controller.signal)
        .then((found) => {
          setResults(found);
          setError(null);
          setIndex(0);
        })
        .catch((e: unknown) => {
          if (controller.signal.aborted) return;
          setResults(null);
          setError(e instanceof Error ? e.message : String(e));
        });
    }, DEBOUNCE_MS);
    return () => {
      window.clearTimeout(timer);
      controller.abort();
    };
  }, [q, deckId, everyColour]);

  // Click outside closes it. The dropdown lives inside `box`, so a click on a
  // row or the footer is not outside.
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!box.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  useHotkeys({
    "/": (e) => {
      e.preventDefault();
      input.current?.focus();
    },
  });

  const rows: AnnotatedResult[] = useMemo(
    () => (results ? annotateResults(results, state, marks) : []),
    [results, state, marks],
  );
  const hasCommander = state.cards.some((c) => c.role === "commander");
  const shown = open && (results !== null || error !== null);
  const highlighted = Math.min(index, Math.max(rows.length - 1, 0));

  // Keep the highlighted row in view as the arrows move through a long list.
  useEffect(() => {
    if (shown) box.current?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: "nearest" });
  }, [shown, highlighted, rows]);

  async function add(r: AnnotatedResult, how: { maybe?: boolean; role?: string } = {}) {
    const { card } = r;
    // Already here: the useful thing is to look at it, not to add a copy.
    if (r.inDeck || r.onMaybe) {
      ui.openCardPanel({ oracleId: card.oracle_id, name: card.name });
      setOpen(false);
      return;
    }
    if (adding.current) return;
    adding.current = true;
    const result = await run(() => api.addCard(deckId, card.oracle_id, how));
    adding.current = false;
    if (!result) return;

    // The add is the newest log entry; undoing it needs that entry's id.
    const entry = result.state.log.reduce<{ id: number } | null>((a, b) => (!a || b.id > a.id ? b : a), null);
    const message = how.maybe
      ? `Added ${card.name} to the maybe list.`
      : how.role === "commander"
        ? `Added ${card.name} as the commander.`
        : `Added ${card.name}. The deck is at ${result.state.computed.card_count} cards.`;
    toast(message, {
      action: entry ? { label: "Undo", onAction: () => void run(() => api.undo(deckId, entry.id)) } : undefined,
    });
    // Ready for the next name: typing replaces what was there.
    input.current?.focus();
    input.current?.select();
  }

  function onKeyDown(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Escape") {
      // This field's Escape is its own; the screen's would also clear the selection.
      e.stopPropagation();
      setOpen(false);
      input.current?.blur();
      return;
    }
    if (rows.length === 0) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      setOpen(true);
      const step = e.key === "ArrowDown" ? 1 : -1;
      setIndex((highlighted + step + rows.length) % rows.length);
    } else if (e.key === "Enter" && shown) {
      e.preventDefault();
      void add(rows[highlighted], e.shiftKey ? { maybe: true } : {});
    }
  }

  return (
    <div ref={box} className={styles.wrap}>
      <label className={styles.bar}>
        <Icon name="search" size={14} className={styles.icon} />
        <input
          ref={input}
          className={styles.input}
          value={query}
          placeholder="Add a card, or search: a name, or o:text t:type mv<=2"
          autoComplete="off"
          spellCheck={false}
          role="combobox"
          aria-label="Add a card or search"
          aria-expanded={shown}
          aria-controls={listId}
          aria-autocomplete="list"
          aria-activedescendant={shown && rows.length ? `${listId}-${highlighted}` : undefined}
          onChange={(e) => {
            setQuery(e.target.value);
            setOpen(true);
          }}
          onFocus={() => setOpen(true)}
          onKeyDown={onKeyDown}
        />
        <Kbd>/</Kbd>
      </label>

      {shown && (
        // Pressing inside must not pull focus out of the input.
        <div className={styles.drop} onMouseDown={(e) => e.target instanceof HTMLInputElement || e.preventDefault()}>
          <div id={listId} role="listbox" className={styles.results}>
            {error ? (
              <ErrorNote>{error}</ErrorNote>
            ) : rows.length === 0 ? (
              <Text tone="muted" as="div" className={styles.none}>
                No card matches. Names resolve exactly; nothing is guessed.
              </Text>
            ) : (
              rows.map((r, i) => (
                <AddBarResult
                  key={r.card.oracle_id}
                  id={`${listId}-${i}`}
                  result={r}
                  highlighted={i === highlighted}
                  offerCommander={!hasCommander && !!r.card.is_commander && !r.inDeck && !r.onMaybe}
                  onPick={(shift) => void add(r, shift ? { maybe: true } : {})}
                  onCommander={() => void add(r, { role: "commander" })}
                  onHighlight={() => setIndex(i)}
                />
              ))
            )}
          </div>
          <div className={styles.foot}>
            <span className={styles.hints}>
              <span><Kbd>↵</Kbd> add to deck</span>
              <span><Kbd>⇧↵</Kbd> add to maybe list</span>
              <span><Kbd>Esc</Kbd> close</span>
            </span>
            <span className={styles.limit}>
              {!everyColour && hasCommander ? (
                <>
                  Limited to the deck's colours <ColorIdentity colors={state.deck.color_identity} />
                </>
              ) : !everyColour ? (
                "No commander yet, so every colour"
              ) : null}
              <Checkbox
                checked={everyColour}
                onChange={(on) => {
                  setEveryColour(on);
                  input.current?.focus();
                }}
                label="Search every colour"
              />
            </span>
          </div>
        </div>
      )}
    </div>
  );
}
