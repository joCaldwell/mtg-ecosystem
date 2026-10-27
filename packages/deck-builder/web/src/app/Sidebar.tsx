import { DeckThumbnail } from "../mtg/index.ts";
import { usePreferences } from "./Preferences.tsx";
import { familyOrder } from "../domain/familyOrder.ts";
// The left rail: where to go, which decks exist, and what an agent connected
// right now would be told. Decks are one click away from anywhere, and a deck
// with something waiting on the owner carries the agent colour.

import { api } from "../api/client.ts";
import { useQuery } from "../api/useQuery.ts";
import { DECK_CATEGORY_LABELS } from "../domain/deckCategory.ts";
import { ago } from "../domain/time.ts";
import { Badge, Dot, Eyebrow, Icon, IconButton } from "../ui/index.ts";
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
  const { waiting } = useInbox();
  const { focus } = useFocus();
  const decks = useQuery(() => api.listDecks(), []);
  const activity = useQuery(() => api.latestActivity(), []);
  const last = activity.data;
  const section = route.page;

  const presence = [last && `${last.actor} · ${ago(last.ts)}`, focus ? `Agent context: ${focus.deckName}${focus.selectedCount > 0 ? ` · ${focus.selectedCount} selected` : ""}` : "No deck in agent context"].filter(Boolean).join(". ");

  return (
    <aside className={`${styles.sidebar} ${collapsed ? styles.collapsed : ""}`} aria-label="Sidebar">
      {brand && <div className={styles.brand}>
        {!collapsed && <><span className={styles.mark} aria-hidden="true" /><span className={styles.brandName}>Deck Builder</span></>}
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
              aria-label={collapsed ? `${deck.name}${pending ? ", waiting on you" : ""}${deck.category !== "paper" ? `, ${DECK_CATEGORY_LABELS[deck.category]}` : ""}${deck.visibility !== "private" ? `, ${deck.visibility}` : ""}` : undefined}
              title={pending ? `${deck.name} — something is waiting on you` : deck.name}
            >
              {/* The thumbnail takes the dot's column; a deck waiting on the owner keeps its violet dot over it. */}
              <span className={styles.lead}>
                <DeckThumbnail cards={deck.commander_art} className={styles.thumb} />
                <Dot tone={pending ? "agent" : "neutral"} className={pending ? styles.pending : styles.fallback} />
              </span>
              <span className={styles.name}>{deck.name}</span>
              {!collapsed && pending && <span className="visually-hidden">, waiting on you</span>}
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
