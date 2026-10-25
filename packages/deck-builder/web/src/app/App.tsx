import { PlaytestPage } from "../features/playtest/index.ts";
import type { ReactNode } from "react";
import { ActivityPage } from "../features/activity/index.ts";
import { DeckPage } from "../features/deck/index.ts";
import { DecksPage } from "../features/decks/index.ts";
import { InboxPage } from "../features/inbox/index.ts";
import { MemoriesPage } from "../features/memories/index.ts";
import { ProfilePage } from "../features/profile/index.ts";
import { SettingsPage } from "../features/settings/index.ts";
import { StyleguidePage } from "../features/styleguide/index.ts";
import { ErrorBoundary } from "./ErrorBoundary.tsx";
import { AppShell } from "./AppShell.tsx";
import { AppProviders } from "./Providers.tsx";
import { useRoute, type Route } from "./routes.ts";

/** The one place a route becomes a screen. A new page is a line here and a
 *  line in routes.ts. */
function screen(route: Route): ReactNode {
  switch (route.page) {
    case "playtest":
      return <PlaytestPage deckId={route.deckId} key={route.deckId} />;
    case "deck":
      return <DeckPage deckId={route.deckId} review={route.review} key={route.deckId} />;
    case "decks":
      return <DecksPage />;
    case "inbox":
      return <InboxPage />;
    case "memories":
      return <MemoriesPage />;
    case "activity":
      return <ActivityPage />;
    case "profile":
      return <ProfilePage />;
    case "settings":
      return <SettingsPage />;
    case "design":
      return <StyleguidePage />;
  }
}

export function App() {
  const route = useRoute();
  return (
    <ErrorBoundary><AppProviders>
      <AppShell route={route}>{screen(route)}</AppShell>
    </AppProviders></ErrorBoundary>
  );
}
