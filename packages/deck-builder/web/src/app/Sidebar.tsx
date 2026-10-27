import { DeckThumbnail } from "../mtg/index.ts";
import { usePreferences } from "./Preferences.tsx";
import { familyOrder } from "../domain/familyOrder.ts";
import { creditedArt } from "../domain/art.ts";
// The left rail: where to go, which decks exist, and what an agent connected
// right now would be told. Decks are one click away from anywhere, and a deck
// with something waiting on the owner carries the agent colour.

import { api, type DeckSummary } from "../api/client.ts";
import { useQuery } from "../api/useQuery.ts";
import { DECK_CATEGORY_LABELS } from "../domain/deckCategory.ts";
import { ago } from "../domain/time.ts";
import { Badge, Dot, Eyebrow, Icon } from "../ui/index.ts";
import { useFocus } from "./Focus.tsx";
import { useInbox } from "./Inbox.tsx";
import { href, type PageName, type Route } from "./routes.ts";
import styles from "./Sidebar.module.css";

const NAV: Array<{ page: PageName; label: string; icon: "inbox" | "decks" | "memories" | "activity" | "profile" | "settings" }> = [
  { page: "inbox", label: "Inbox", icon: "inbox" },
  { page: "decks", label: "All decks", icon: "decks" },
  { page: "memories", label: "Memories", icon: "memories" },
  { page: "activity", label: "Activity", icon: "activity" },
  { page: "profile", label: "Profile", icon: "profile" },
  { page: "settings", label: "Settings", icon: "settings" },
];


/** `brand` is off inside the tablet drawer, whose title already names it. */
export function Sidebar({ route, brand = true }: { route: Route; brand?: boolean }) {
  const { hasAgentActivity } = usePreferences();
  const { waiting } = useInbox();
  const { focus } = useFocus();
  const decks = useQuery(() => api.listDecks(), []);
  const activity = useQuery(() => api.latestActivity(), []);
  const last = activity.data;

  return (
    <aside className={styles.sidebar}>
      {brand && <div className={styles.brand}>
        <span className={styles.mark} aria-hidden="true" />
        Deck Builder
      </div>}

      <nav className={styles.nav} aria-label="Sections">
        {NAV.map((item) => (
          <a
            key={item.page}
            href={href({ page: item.page })}
            className={route.page === item.page ? styles.current : undefined}
            aria-current={route.page === item.page ? "page" : undefined}
          >
            <Icon name={item.icon} />
            {item.label}
            {item.page === "inbox" && waiting > 0 && (
              <span className={styles.end}>
                <Badge tone="agent">{waiting}</Badge>
              </span>
            )}
          </a>
        ))}
      </nav>

      <div className={styles.heading}>
        <Eyebrow>Decks</Eyebrow>
      </div>
      <nav className={styles.decks} aria-label="Decks">
        {familyOrder(decks.data ?? []).map(({ deck, fork }) => {
          const open = (route.page === "deck" || route.page === "playtest") && route.deckId === deck.id;
          const pending = deck.pending_items + deck.pending_document_edits > 0;
          return (
            <a
              key={deck.id}
              href={href({ page: "deck", deckId: deck.id })}
              className={[open ? styles.current : "", fork ? styles.fork : ""].join(" ")}
              aria-current={open ? "page" : undefined}
              title={pending ? `${deck.name} — something is waiting on you` : deck.name}
            >
              {/* The thumbnail takes the dot's column; a deck waiting on the owner keeps its violet dot over it. */}
              <span className={styles.lead}>
                <DeckThumbnail cards={deck.commander_art} className={styles.thumb} />
                {(pending || !creditedArt(deck.commander_art).length) && <Dot tone={pending ? "agent" : "neutral"} />}
              </span>
              <span className={styles.name}>{deck.name}</span>
              {deck.visibility !== "private" && (
                <span className={styles.shared} role="img" aria-label={deck.visibility === "public" ? "Public" : "Unlisted"} title={deck.visibility === "public" ? "Public" : "Unlisted: anyone with the link"}>
                  <Icon name={deck.visibility === "public" ? "globe" : "link"} size={12} />
                </span>
              )}
              {deck.category !== "paper" && (
                <span className={styles.lifecycle} data-lifecycle={deck.category} title={DECK_CATEGORY_LABELS[deck.category]}>
                  {deck.category === "theorycrafted" ? "Theory" : DECK_CATEGORY_LABELS[deck.category]}
                </span>
              )}
            </a>
          );
        })}
      </nav>

      {hasAgentActivity && <div className={styles.presence}>
        {last ? (
          <div className={styles.who}>
            <Dot tone="agent" pulse />
            <span className={styles.agent}>{last.actor}</span>
            <span className={styles.when}>· {ago(last.ts)}</span>
          </div>
        ) : (
          <div className={styles.when}>No agent has written anything yet.</div>
        )}
        <div className={styles.focus}>
          {focus ? (
            <>
              To an agent, “this deck” is <b>{focus.deckName}</b>
              {focus.selectedCount > 0 && (
                <>
                  {" "}
                  and “these cards” are the <b>{focus.selectedCount} selected</b>
                </>
              )}
              .
            </>
          ) : (
            <>No deck is open, so “this deck” means nothing to an agent right now.</>
          )}
        </div>
      </div>}
    </aside>
  );
}
