// The left rail: where to go, which decks exist, and what an agent connected
// right now would be told. Decks are one click away from anywhere, and a deck
// with something waiting on the owner carries the agent colour.

import { api, type DeckSummary } from "../api/client.ts";
import { useQuery } from "../api/useQuery.ts";
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

/** Forks listed directly under the deck they came from. */
function familyOrder(decks: DeckSummary[]): Array<{ deck: DeckSummary; fork: boolean }> {
  const ids = new Set(decks.map((d) => d.id));
  const roots = decks.filter((d) => d.parent_id == null || !ids.has(d.parent_id));
  return roots.flatMap((root) => [
    { deck: root, fork: false },
    ...decks.filter((d) => d.parent_id === root.id).map((deck) => ({ deck, fork: true })),
  ]);
}

export function Sidebar({ route }: { route: Route }) {
  const { waiting } = useInbox();
  const { focus } = useFocus();
  const decks = useQuery(() => api.listDecks(), []);
  const activity = useQuery(() => api.listActivity(), []);
  const last = activity.data?.[0];

  return (
    <aside className={styles.sidebar}>
      <div className={styles.brand}>
        <span className={styles.mark} aria-hidden="true" />
        Deck Builder
      </div>

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
              <Dot tone={pending ? "agent" : "neutral"} />
              <span className={styles.name}>{deck.name}</span>
              {deck.category === "legacy" && <span className={styles.draft}>legacy</span>}
            </a>
          );
        })}
      </nav>

      <div className={styles.presence}>
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
      </div>
    </aside>
  );
}
