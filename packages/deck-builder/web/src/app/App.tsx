import { PlaytestPage } from "../features/playtest/index.ts";
import type { ReactNode } from "react";
import { ActivityPage } from "../features/activity/index.ts";
import { DeckPage } from "../features/deck/index.ts";
import { DecksPage } from "../features/decks/index.ts";
import { InboxPage } from "../features/inbox/index.ts";
import { MemoriesPage } from "../features/memories/index.ts";
import { PhoneApp } from "../features/phone/index.ts";
import { ProfilePage } from "../features/profile/index.ts";
import { PublicDeckPage, PublicProfilePage } from "../features/public/index.ts";
import { SettingsPage } from "../features/settings/index.ts";
import { StyleguidePage } from "../features/styleguide/index.ts";
import { EmptyState, Page, ToastProvider } from "../ui/index.ts";
import { ErrorBoundary } from "./ErrorBoundary.tsx";
import { AppShell } from "./AppShell.tsx";
import { useIsPhone } from "./breakpoints.ts";
import { AppProviders, CardProviders } from "./Providers.tsx";
import { SessionProvider } from "./Session.tsx";
import { href, isPublicRoute, useRoute, type Route } from "./routes.ts";

/** The one place a route becomes a screen. A new page is a line here and a
 *  line in routes.ts. */
function screen(route: Route): ReactNode {
  switch (route.page) {
    case "playtest":
      return <PlaytestPage deckId={route.deckId} key={route.deckId} />;
    case "deck":
      return <DeckPage deckId={route.deckId} review={route.review} key={route.deckId} />;
    case "decks":
    case "signin":
    case "privacy":
    case "terms":
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
    case "shared":
    case "user":
    case "missing":
      return <Page><EmptyState title="There is nothing at this address">Check the link, or go <a href={href({ page: "decks" })}>back to all decks</a>.</EmptyState></Page>;
  }
}

/** Below 720 px the signed-in app is the read-and-rule phone view (D17). */
function SignedIn({ route }: { route: Route }) {
  const phone = useIsPhone();
  return phone ? <PhoneApp route={route} /> : <AppShell route={route}>{screen(route)}</AppShell>;
}

export function App() {
  const route = useRoute();
  const open = isPublicRoute(route);
  return (
    <ErrorBoundary><ToastProvider>
      <SessionProvider open={open}>
        {route.page === "shared" ? <CardProviders><PublicDeckPage slug={route.slug} key={route.slug} /></CardProviders>
          : route.page === "user" ? <CardProviders><PublicProfilePage handle={route.handle} key={route.handle} /></CardProviders>
          : <AppProviders><SignedIn route={route} /></AppProviders>}
      </SessionProvider>
    </ToastProvider></ErrorBoundary>
  );
}
