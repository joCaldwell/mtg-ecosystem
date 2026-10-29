import { useLayoutEffect, useMemo, useRef, useState } from "react";
import { DeckThumbnail } from "../mtg/index.ts";
import { BrandMark } from "./BrandMark.tsx";
import { usePreferences } from "./Preferences.tsx";
import { familyOrder } from "../domain/familyOrder.ts";
import { waitingByDeck } from "../domain/inbox.ts";
// The left rail: where to go, which decks exist, and what an agent connected
// right now would be told. Decks are one click away from anywhere, and a deck
// with something waiting on the owner carries the agent colour.

import { api } from "../api/client.ts";
import { useQuery } from "../api/useQuery.ts";
import { DECK_CATEGORY_LABELS } from "../domain/deckCategory.ts";
import { ago } from "../domain/time.ts";
import { Badge, Dot, Eyebrow, Icon, IconButton, TextField } from "../ui/index.ts";
import { useFocus } from "./Focus.tsx";
import { useInbox } from "./Inbox.tsx";
import { href, type PageName, type Route } from "./routes.ts";
import styles from "./Sidebar.module.css";

const NAV: Array<{ page: PageName; label: string; icon: "inbox" | "decks" | "memories" | "activity" | "profile" | "settings" }> = [
  { page: "decks", label: "All decks", icon: "decks" },
  { page: "inbox", label: "Inbox", icon: "inbox" },
  { page: "memories", label: "Memories", icon: "memories" },
  { page: "activity", label: "Activity", icon: "activity" },
  { page: "profile", label: "Profile", icon: "profile" },
  { page: "settings", label: "Settings", icon: "settings" },
];


/** `brand` is off inside the tablet drawer, whose title already names it. */
export function Sidebar({ route, brand = true, collapsed = false, onToggle, toggleDisabled = false }: {
  route: Route; brand?: boolean; collapsed?: boolean; onToggle?: () => void; toggleDisabled?: boolean;
}) {
  const { hasAgentActivity } = usePreferences();
  const { waiting, inbox } = useInbox();
  const deckWaiting = useMemo(() => inbox ? waitingByDeck(inbox) : new Map<number, number>(), [inbox]);
  const { focus } = useFocus();
  const decks = useQuery(() => api.listDecks(), []);
  const activity = useQuery(() => api.latestActivity(), []);
  const [deckQuery, setDeckQuery] = useState("");
  const filterInput = useRef<HTMLInputElement>(null);
  const deckNav = useRef<HTMLElement>(null);
  const canFilter = brand && !collapsed && (decks.data?.length ?? 0) > 8;
  const query = canFilter ? deckQuery.trim().toLocaleLowerCase() : "";
  const orderedDecks = familyOrder(decks.data ?? []);
  const shownDecks = orderedDecks.filter(({ deck }) => !query || [deck.name, ...deck.commanders].some(name => name.toLocaleLowerCase().includes(query)));
  const currentDeckId = route.page === "deck" || route.page === "playtest" ? route.deckId : null;
  useLayoutEffect(() => {
    if (!brand || currentDeckId == null) return;
    const nav = deckNav.current;
    if (!nav) return;
    const reveal = () => {
      const current = nav.querySelector<HTMLElement>('a[aria-current="page"]');
      if (!current) return false; // The summaries may still be loading.
      const pane = nav.getBoundingClientRect(), item = current.getBoundingClientRect();
      // Only this rail scrolls. A later presence footer can reduce its height.
      if (item.bottom > pane.bottom) nav.scrollTop += item.bottom - pane.bottom;
      else if (item.top < pane.top) nav.scrollTop += item.top - pane.top;
      return true;
    };
    const waiting = new MutationObserver(() => { if (reveal()) waiting.disconnect(); });
    waiting.observe(nav, { childList: true, subtree: true });
    if (reveal()) waiting.disconnect();
    const sizes = new ResizeObserver(() => { reveal(); });
    sizes.observe(nav);
    return () => { waiting.disconnect(); sizes.disconnect(); };
  }, [brand, collapsed, currentDeckId, route.page]);
  const last = activity.data;
  const section = route.page;

  const presence = [last && `${last.actor} · ${ago(last.ts)}`, focus ? `Agent context: ${focus.deckName}${focus.selectedCount > 0 ? ` · ${focus.selectedCount} selected` : ""}` : "No deck in agent context"].filter(Boolean).join(". ");

  return (
    <aside className={`${styles.sidebar} ${collapsed ? styles.collapsed : ""}`} aria-label="Sidebar">
      {brand && <div className={styles.brand}>
        <span className={styles.brandName}><BrandMark className={styles.mark} />{!collapsed && "Deck Builder"}</span>
        {onToggle && <IconButton label={collapsed ? "Expand sidebar" : "Collapse sidebar"} aria-expanded={!collapsed} onClick={onToggle} disabled={toggleDisabled} className={styles.toggle}>
          <Icon name="back" className={collapsed ? styles.expandIcon : undefined} />
        </IconButton>}
      </div>}

      <nav className={styles.nav} aria-label="Sections">
        {NAV.map((item) => (
          <a
            key={item.page}
            href={href({ page: item.page })}
            className={section === item.page ? styles.current : undefined}
            aria-current={section === item.page ? "page" : undefined}
            aria-label={collapsed ? (item.page === "inbox" && waiting > 0 ? `Inbox, ${waiting} waiting` : item.label) : undefined}
            title={collapsed ? item.label : undefined}
          >
            <Icon name={item.icon} />
            <span className={styles.label}>{item.label}</span>
            {item.page === "inbox" && waiting > 0 && (
              <span className={styles.end}>
                <Badge className={styles.inboxBadge}>{waiting}</Badge>{!collapsed && <span className="visually-hidden"> waiting</span>}
              </span>
            )}
          </a>
        ))}
      </nav>

      <div className={styles.heading} aria-hidden={collapsed || undefined}>
        <Eyebrow>Your decks</Eyebrow>
        {query && <span className={styles.matches} role="status">{shownDecks.length} {shownDecks.length === 1 ? "match" : "matches"}</span>}
      </div>
      {canFilter && <div className={styles.filter}>
        <TextField ref={filterInput} type="search" aria-label="Find a deck or commander in sidebar" placeholder="Find a deck…"
          value={deckQuery} onChange={event => setDeckQuery(event.target.value)}
          onKeyDown={event => { if (event.key === "Escape" && deckQuery) { event.preventDefault(); event.stopPropagation(); setDeckQuery(""); } }} />
      </div>}
      <nav ref={deckNav} className={styles.decks} aria-label="Decks">
        {shownDecks.map(({ deck, fork }) => {
          const open = (route.page === "deck" || route.page === "playtest") && route.deckId === deck.id;
          // The count matches the Inbox badge (proposals and edit groups); the
          // summary's own flag covers the moment before the inbox has loaded.
          const count = deckWaiting.get(deck.id) ?? 0;
          const pending = count > 0 || deck.pending_items + deck.pending_document_edits > 0;
          const waitingText = count > 0 ? `${count} ${count === 1 ? "proposal" : "proposals"} waiting` : "waiting on you";
          return (
            <a
              key={deck.id}
              href={href({ page: "deck", deckId: deck.id })}
              className={[open ? styles.current : "", fork ? styles.fork : ""].join(" ")}
              aria-current={open ? "page" : undefined}
              aria-label={collapsed ? `${deck.name}${pending ? `, ${waitingText}` : ""}${deck.category !== "paper" ? `, ${DECK_CATEGORY_LABELS[deck.category]}` : ""}${deck.visibility !== "private" ? `, ${deck.visibility}` : ""}` : undefined}
              title={`${deck.name}${deck.category !== "paper" ? ` — ${DECK_CATEGORY_LABELS[deck.category]}` : ""}${pending ? ` — ${waitingText}` : ""}`}
            >
              {/* The thumbnail takes the dot's column. Expanded rows count what waits at
                  their end, as the Inbox does; the rail has no room, so a waiting deck
                  keeps a violet dot on the thumbnail's corner there. */}
              <span className={styles.lead}>
                <DeckThumbnail cards={deck.commander_art} className={styles.thumb} />
                <Dot tone={collapsed && pending ? "agent" : "neutral"} className={collapsed && pending ? styles.pending : styles.fallback} />
              </span>
              <span className={styles.name}>{deck.name}</span>
              {!collapsed && pending && (count > 0
                ? <span className={styles.waiting}><Badge aria-hidden="true" className={styles.waitingBadge}>{count}</Badge><span className="visually-hidden">, {waitingText}</span></span>
                : <span className="visually-hidden">, {waitingText}</span>)}
              {deck.visibility !== "private" && (
                <span className={styles.shared} role="img" aria-label={deck.visibility === "public" ? "Public" : "Unlisted"} title={deck.visibility === "public" ? "Public" : "Unlisted: anyone with the link"}>
                  <Icon name={deck.visibility === "public" ? "globe" : "link"} size={12} />
                </span>
              )}
              {deck.category !== "paper" && (
                <span className={styles.lifecycle} data-lifecycle={deck.category}>
                  {DECK_CATEGORY_LABELS[deck.category]}
                </span>
              )}
            </a>
          );
        })}
        {query && shownDecks.length === 0 && <div className={styles.noMatches}>
          <span>No decks found</span>
          <button type="button" onClick={() => { setDeckQuery(""); filterInput.current?.focus(); }}>Clear filter</button>
        </div>}
      </nav>

      {hasAgentActivity && <div className={styles.presence} title={presence}>
        {collapsed && <span className="visually-hidden">{presence}</span>}
        <div className={styles.who}>
          <Dot tone="agent" />
          <span className={styles.presenceText}>
            <span className={styles.agent}>{last?.actor ?? "Agent activity"}</span>
            {last && <span className={styles.when}> · {ago(last.ts)}</span>}
          </span>
        </div>
        {!collapsed && <div className={styles.focus}>
          {focus ? <><span>Agent context</span><b>{focus.deckName}</b>{focus.selectedCount > 0 && <span>{focus.selectedCount} selected</span>}</> : "No deck in agent context"}
        </div>}
      </div>}
    </aside>
  );
}
