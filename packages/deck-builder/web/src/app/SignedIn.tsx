import { lazy, type ReactNode } from "react";
import { ActivityPage } from "../features/activity/index.ts";
import { DeckPage } from "../features/deck/index.ts";
import { DecksPage } from "../features/decks/index.ts";
import { InboxPage } from "../features/inbox/index.ts";
import { MemoriesPage } from "../features/memories/index.ts";
import { PhoneApp } from "../features/phone/index.ts";
import { ProfilePage } from "../features/profile/index.ts";
import { EmptyState, Page } from "../ui/index.ts";
import { AppShell } from "./AppShell.tsx";
import { useIsPhone } from "./breakpoints.ts";
import { href, type Route } from "./routes.ts";

import { useMediaQuery, PANEL_QUERY } from "./breakpoints.ts";
const PlaytestPage = lazy(() => import("../features/playtest/index.ts").then(m => ({ default: m.PlaytestPage })));
const SettingsPage = lazy(() => import("../features/settings/index.ts").then(m => ({ default: m.SettingsPage })));
const StyleguidePage = lazy(() => import("../features/styleguide/index.ts").then(m => ({ default: m.StyleguidePage })));

/** The one place a route becomes a screen. A new page is a line here and a
 *  line in routes.ts. */
function screen(route: Route): ReactNode {
  switch (route.page) {
    case "playtest":
      return <DesktopPlaytest deckId={route.deckId} key={route.deckId} />;
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
export function SignedIn({ route }: { route: Route }) {
  const phone = useIsPhone();
  return phone ? <PhoneApp route={route} /> : <AppShell route={route}>{screen(route)}</AppShell>;
}

function DesktopPlaytest({ deckId }: { deckId: number }) {
  const narrow = useMediaQuery(PANEL_QUERY);
  return narrow ? <Page><EmptyState title="Open on a larger screen">The playtester needs a desktop window at least 1100 px wide. <a href={href({ page: "deck", deckId })}>Back to the deck</a></EmptyState></Page> : <PlaytestPage deckId={deckId} />;
}