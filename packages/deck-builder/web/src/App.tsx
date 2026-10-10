import { useEffect, useState } from "react";
import { api } from "./api.ts";
import { ActivityPage } from "./ActivityPage.tsx";
import { CardPeekProvider } from "./CardPeek.tsx";
import { DeckList } from "./DeckList.tsx";
import { DeckView } from "./DeckView.tsx";
import { InboxPage } from "./InboxPage.tsx";
import { LiveProvider, useLive } from "./live.tsx";
import { MemoriesPage } from "./MemoriesPage.tsx";
import { ProfilePage } from "./ProfilePage.tsx";
import { SettingsPage } from "./SettingsPage.tsx";

type Route =
  | { page: "decks" | "inbox" | "memories" | "profile" | "activity" | "settings" }
  | { page: "deck"; deckId: number };

function parseHash(): Route {
  const hash = window.location.hash;
  const deck = hash.match(/^#\/deck\/(\d+)$/);
  if (deck) return { page: "deck", deckId: Number(deck[1]) };
  const page = hash.match(/^#\/(inbox|memories|profile|activity|settings)$/);
  return { page: (page?.[1] as "inbox") ?? "decks" };
}

const NAV: Array<{ page: Route["page"]; label: string; href: string }> = [
  { page: "decks", label: "Decks", href: "#/" },
  { page: "inbox", label: "Inbox", href: "#/inbox" },
  { page: "memories", label: "Memories", href: "#/memories" },
  { page: "profile", label: "Profile", href: "#/profile" },
  { page: "activity", label: "Activity", href: "#/activity" },
  { page: "settings", label: "Settings", href: "#/settings" },
];

function Nav({ route }: { route: Route }) {
  const live = useLive();
  const [waiting, setWaiting] = useState(0);
  useEffect(() => {
    api
      .getInbox()
      .then((inbox) =>
        setWaiting(
          inbox.proposals.reduce((n, p) => n + p.items.filter((i) => i.status === "pending").length, 0) +
            inbox.document_edits.length,
        ),
      )
      .catch(() => {});
  }, [live]);
  return (
    <nav className="app-nav" aria-label="Sections">
      <span className="app-name">Deck builder</span>
      {NAV.map((n) => (
        <a
          key={n.page}
          href={n.href}
          className={route.page === n.page || (n.page === "decks" && route.page === "deck") ? "active" : ""}
        >
          {n.label}
          {n.page === "inbox" && waiting > 0 && <span className="tool-count">{waiting}</span>}
        </a>
      ))}
    </nav>
  );
}

export function App() {
  const [route, setRoute] = useState<Route>(parseHash());

  useEffect(() => {
    const onHash = () => setRoute(parseHash());
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);

  // Above the router: the popover is anchored to the viewport and outlives
  // nothing, but every surface that names a card sits underneath here.
  return (
    <LiveProvider>
      <CardPeekProvider>
        <Nav route={route} />
        {route.page === "deck" && <DeckView deckId={route.deckId} key={route.deckId} />}
        {route.page === "decks" && <DeckList />}
        {route.page === "inbox" && <InboxPage />}
        {route.page === "memories" && <MemoriesPage />}
        {route.page === "profile" && <ProfilePage />}
        {route.page === "activity" && <ActivityPage />}
        {route.page === "settings" && <SettingsPage />}
      </CardPeekProvider>
    </LiveProvider>
  );
}
