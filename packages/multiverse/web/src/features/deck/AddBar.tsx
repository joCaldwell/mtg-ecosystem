// Add a card, or search for one (spec §4). One input does both: a name that
// resolves exactly is added; anything else is a query, and its results can be
// added from the list. Nothing is guessed; "No card matches" is the answer
// to a name that is not a card.

import { useEffect, useId, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { api, type CardData } from "../../api/client.ts";
import { annotateResults, type AnnotatedResult } from "../../domain/search.ts";
import { ColorIdentity } from "../../mtg/index.ts";
import { Button, Checkbox, ErrorNote, Icon, Kbd, placeNear, Text, tokenPx, useHotkeys, useToast } from "../../ui/index.ts";
import { AddBarResult } from "./AddBarResult.tsx";
import { useDeck, useDeckState } from "./DeckStore.tsx";
import { useDeckUi } from "./DeckUi.tsx";
import { useSharedDeckView } from "./DeckViewContext.ts";
import { revealCardRow } from "./list/DeckList.tsx";
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
  const [response, setResponse] = useState<{
    key: string;
    results: CardData[] | null;
    error: string | null;
  } | null>(null);
  const [open, setOpen] = useState(false);
  const [index, setIndex] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const box = useRef<HTMLDivElement>(null);
  const drop = useRef<HTMLDivElement>(null);
  const resultList = useRef<HTMLDivElement>(null);
  const adding = useRef(false);
  const interaction = useRef(0);
  const [pending, setPending] = useState(false);
  const listId = useId();
  const anchorName = `--add-bar-${listId.replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const nativePopover = typeof HTMLElement !== "undefined" && Object.hasOwn(HTMLElement.prototype, "popover");
  const anchored = nativePopover && typeof CSS !== "undefined" && CSS.supports("position-area: block-end") && CSS.supports("position-try-order: most-block-size");

  const q = query.trim();
  const hasCommander = state.cards.some((c) => c.role === "commander");
  const identityScope = everyColour ? null : `${hasCommander}:${state.deck.color_identity}`;
  // The search is limited to the commander's colours unless asked otherwise.
  const limited = !everyColour && hasCommander;
  const queryKey = JSON.stringify([deckId, q, everyColour, identityScope]);
  // A changed query or colour scope invalidates the previous options at render
  // time, before the debounce/request runs. Enter cannot pick a stale card.
  const current = response?.key === queryKey;
  const results = current ? response.results : null;
  const error = current ? response.error : null;
  const loading = q.length >= MIN_QUERY && !current;

  // Search as the owner types. Each keystroke supersedes the last: its timer
  // is cleared and its request aborted, so a slow answer to "dra" can never
  // overwrite the answer to "dragon".
  useEffect(() => {
    if (q.length < MIN_QUERY) {
      setResponse(null);
      return;
    }
    const controller = new AbortController();
    const timer = window.setTimeout(() => {
      api
        .search(q, deckId, !everyColour, controller.signal)
        .then((found) => {
          if (controller.signal.aborted) return;
          setResponse({ key: queryKey, results: found, error: null });
          setIndex(0);
        })
        .catch((e: unknown) => {
          if (controller.signal.aborted) return;
          setResponse({ key: queryKey, results: null, error: e instanceof Error ? e.message : String(e) });
        });
    }, DEBOUNCE_MS);
    return () => {
      window.clearTimeout(timer);
      controller.abort();
    };
  }, [q, deckId, everyColour, identityScope]);

  function close() {
    interaction.current++;
    setOpen(false);
  }

  function setColourScope(on: boolean) {
    interaction.current++;
    setEveryColour(on);
    input.current?.focus({ preventScroll: true });
  }

  // Keep the dropdown in the field's DOM order for Tab, including without
  // the Popover API. Pointer and focus departures dismiss the surface.
  useEffect(() => {
    const anchor = box.current;
    if (!open || !anchor) return;
    const onDown = (e: PointerEvent) => {
      if (!anchor.contains(e.target as Node)) close();
    };
    const observer = new IntersectionObserver(([entry]) => { if (!entry.isIntersecting) close(); });
    observer.observe(anchor);
    document.addEventListener("pointerdown", onDown);
    return () => {
      observer.disconnect();
      document.removeEventListener("pointerdown", onDown);
    };
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
  const shown = open && (loading || results !== null || error !== null);
  const highlighted = Math.min(index, Math.max(rows.length - 1, 0));
  // The highlighted card is already here: Enter views it instead of adding.
  const viewing = !!rows[highlighted] && (rows[highlighted].inDeck || rows[highlighted].onMaybe);
  // No match inside the commander's colours: the empty state offers the
  // wider search as a button, in place of the footer's checkbox.
  const offerEveryColour = limited && !loading && !error && results !== null && rows.length === 0;

  useLayoutEffect(() => {
    const source = box.current;
    const panel = drop.current;
    if (!shown || !source || !panel) return;
    if (nativePopover) panel.showPopover();
    const hide = () => { if (nativePopover && panel.matches(":popover-open")) panel.hidePopover(); };
    if (anchored) return hide;

    const gap = tokenPx("--s-2");
    const margin = tokenPx("--s-4");
    const minimum = tokenPx("--add-drop-min-w");
    const place = () => {
      const rect = source.getBoundingClientRect();
      panel.style.width = `${Math.max(rect.width, minimum)}px`;
      const room = Math.max(rect.top, innerHeight - rect.bottom) - gap - margin;
      panel.style.maxHeight = `${Math.max(0, room)}px`;
      const { left, top } = placeNear(rect, panel.getBoundingClientRect(), { gap, margin });
      Object.assign(panel.style, { left: `${left}px`, top: `${top}px`, visibility: "visible" });
    };
    place();
    const observer = new ResizeObserver(place);
    observer.observe(source);
    observer.observe(panel);
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
      hide();
    };
  }, [shown, anchored, nativePopover]);

  // Keep the highlighted row in view as the arrows move through a long list.
  // Only the results scroll: scrollIntoView could also move the window,
  // which would close the results.
  useEffect(() => {
    const list = resultList.current;
    const row = shown ? list?.querySelector('[aria-selected="true"]') : null;
    if (!list || !row) return;
    const r = row.getBoundingClientRect();
    const c = list.getBoundingClientRect();
    if (r.top < c.top) list.scrollTop -= c.top - r.top;
    else if (r.bottom > c.bottom) list.scrollTop += r.bottom - c.bottom;
  }, [shown, highlighted, rows]);

  async function add(r: AnnotatedResult, how: { maybe?: boolean; role?: string } = {}) {
    const { card } = r;
    // Already here: the useful thing is to look at it, not to add a copy.
    if (r.inDeck || r.onMaybe) {
      ui.openCardPanel({ oracleId: card.oracle_id, name: card.name });
      close();
      return;
    }
    if (adding.current) return;
    adding.current = true;
    setPending(true);
    const started = interaction.current;
    const invoker = document.activeElement;
    const result = await run(() => api.addCard(deckId, card.oracle_id, how));
    adding.current = false;
    setPending(false);
    if (!result) return;

    // Deck membership adds create a log entry. Shortlisting is organization
    // outside the 100, so its undo removes the newly shortlisted card directly.
    const entry = result.state.log.reduce<{ id: number } | null>((a, b) => (!a || b.id > a.id ? b : a), null);
    const message = how.maybe
      ? `Added ${card.name} to the maybe list.`
      : how.role === "commander"
        ? `Added ${card.name} as the commander.`
        : `Added ${card.name}. The deck is at ${result.state.computed.card_count} cards.`;
    const undo = how.maybe
      ? () => api.removeCard(deckId, card.oracle_id)
      : entry ? () => api.undo(deckId, entry.id) : undefined;
    toast(message, {
      action: undo ? { label: "Undo", onAction: () => void run(undo) } : undefined,
      // A long maybe list hides the new card; this finds it for the owner.
      secondaryAction: how.maybe ? {
        label: "View card",
        onAction: () => {
          revealCardRow(document, card.oracle_id);
          ui.openCardPanel({ oracleId: card.oracle_id, name: card.name });
        },
      } : undefined,
    });
    // Keep shortlist results and their updated membership marks ready for the
    // next card. Selecting the query lets typing replace it immediately.
    if (started !== interaction.current || invoker !== document.activeElement || !box.current?.contains(invoker)) return;
    input.current?.focus({ preventScroll: true });
    if (how.maybe) {
      input.current?.select();
      setOpen(true);
      return;
    }
    setQuery("");
    setResponse(null);
    setIndex(0);
    close();
  }

  function onKeyDown(e: KeyboardEvent<HTMLInputElement>) {
    if (e.nativeEvent.isComposing || e.nativeEvent.keyCode === 229) return;
    if (rows.length === 0) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      interaction.current++;
      setOpen(true);
      const step = e.key === "ArrowDown" ? 1 : -1;
      setIndex((highlighted + step + rows.length) % rows.length);
    } else if (e.key === "Enter" && shown) {
      e.preventDefault();
      void add(rows[highlighted], e.shiftKey ? { maybe: true } : {});
    }
  }

  return (
    // Focus leaving for anything outside the bar closes the results. A press
    // inside them keeps focus in the field (below), and the colour checkbox
    // is inside, so neither counts.
    <div
      ref={box}
      className={styles.wrap}
      style={anchored ? { anchorName } : undefined}
      onBlur={(e) => { if (!box.current?.contains(e.relatedTarget as Node | null)) close(); }}
      onKeyDown={(e) => {
        if (e.key !== "Escape" || e.nativeEvent.isComposing || e.nativeEvent.keyCode === 229) return;
        e.preventDefault(); // Keep native popover and screen cancellation out of this field.
        e.stopPropagation();
        input.current?.focus({ preventScroll: true });
        close();
      }}
    >
      <label className={styles.bar}>
        <span className={styles.addLabel}><Icon name="plus" size={14} /> Add cards</span>
        <input
          ref={input}
          className={styles.input}
          value={query}
          placeholder="Card name or query…"
          autoComplete="off"
          autoCapitalize="none"
          autoCorrect="off"
          enterKeyHint="search"
          spellCheck={false}
          role="combobox"
          aria-label="Add cards"
          aria-expanded={shown && rows.length > 0}
          aria-controls={shown && rows.length > 0 ? listId : undefined}
          aria-autocomplete="list"
          aria-activedescendant={shown && rows.length ? `${listId}-${highlighted}` : undefined}
          onChange={(e) => {
            interaction.current++;
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
        <div
          ref={drop}
          popover={nativePopover ? "manual" : undefined}
          data-anchored={anchored || undefined}
          style={anchored ? { positionAnchor: anchorName } : undefined}
          className={styles.drop}
          onMouseDown={(e) => { if (e.target instanceof Element && e.target.closest('[role="option"]')) e.preventDefault(); }}
        >
          <div ref={resultList} id={listId} tabIndex={-1} role={rows.length > 0 ? "listbox" : undefined} aria-label={rows.length > 0 ? "Card search results" : undefined} className={styles.results}>
            {loading ? (
              <Text role="status" tone="muted" as="div" className={styles.none}>Searching…</Text>
            ) : error ? (
              <ErrorNote>{error}</ErrorNote>
            ) : rows.length === 0 ? (
              <div className={styles.none}>
                <Text role="status" tone="muted">
                  {limited ? "No cards match this search in the deck's colours." : "No cards match this search."}
                </Text>
                {offerEveryColour && (
                  <Button size="sm" onClick={() => setColourScope(true)}>
                    Search every colour
                  </Button>
                )}
                {/* The syntax only where it helps: when nothing came back. */}
                <Text tone="faint" size="xs" as="p" className={styles.syntax}>Search by name, or use o:text t:type mv&lt;=2.</Text>
              </div>
            ) : (
              rows.map((r, i) => (
                <AddBarResult
                  key={r.card.oracle_id}
                  id={`${listId}-${i}`}
                  result={r}
                  highlighted={i === highlighted}
                  onPick={(shift) => void add(r, shift ? { maybe: true } : {})}
                  onHighlight={() => setIndex(i)}
                />
              ))
            )}
          </div>
          {!loading && !error && rows[highlighted] && (
            // Listbox options cannot expose nested buttons. Keep every action
            // in an adjacent, tabbable group for the current result instead.
            <div className={styles.actions} role="group" aria-label={`Actions for ${rows[highlighted].card.name}`} aria-busy={pending}>
              {viewing ? (
                <Button size="sm" onClick={() => void add(rows[highlighted])}>View card</Button>
              ) : (
                <>
                  <Button size="sm" aria-disabled={pending} onClick={() => void add(rows[highlighted])}>Add</Button>
                  <Button size="sm" aria-disabled={pending} kbd="⇧↵" onClick={() => void add(rows[highlighted], { maybe: true })}>Add to maybe</Button>
                  {!hasCommander && !!rows[highlighted].card.is_commander && (
                    <Button size="sm" aria-disabled={pending} onClick={() => void add(rows[highlighted], { role: "commander" })}>Add as commander</Button>
                  )}
                </>
              )}
            </div>
          )}
          <div className={styles.foot}>
            <span className={styles.hints}>
              {rows.length > 0 && !loading && !error && (viewing ? (
                <span><Kbd>↵</Kbd> view</span>
              ) : (
                <>
                  <span><Kbd>↵</Kbd> add</span>
                  <span><Kbd>⇧↵</Kbd> maybe list</span>
                </>
              ))}
              <span><Kbd>Esc</Kbd> close</span>
            </span>
            <span className={styles.limit}>
              {!hasCommander ? (
                "No commander yet, so every colour"
              ) : (
                <>
                  {/* The empty state's button does the same; one offer at a time. */}
                  {!everyColour && <ColorIdentity colors={state.deck.color_identity} />}
                  {!offerEveryColour && (
                    <Checkbox
                      checked={everyColour}
                      onChange={setColourScope}
                      label="Search every colour"
                    />
                  )}
                </>
              )}
            </span>
          </div>
        </div>
      )}
    </div>
  );
}
