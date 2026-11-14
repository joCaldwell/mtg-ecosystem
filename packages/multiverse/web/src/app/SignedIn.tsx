import { lazy, type ReactNode } from "react";
import { EmptyState, Page } from "../ui/index.ts";
import { AppShell } from "./AppShell.tsx";
import { useIsPhone } from "./breakpoints.ts";
import { href, type Route } from "./routes.ts";

import { useMediaQuery, PANEL_QUERY } from "./breakpoints.ts";
// Every screen is a route boundary. Visiting Home must not download the editor,
// review workflow, or phone-only composition before the owner asks for them.
const DeckPage = lazy(() => import("../features/deck/index.ts").then(m => ({ default: m.DeckPage })));
const DecksPage = lazy(() => import("../features/decks/index.ts").then(m => ({ default: m.DecksPage })));
const InboxPage = lazy(() => import("../features/inbox/index.ts").then(m => ({ default: m.InboxPage })));
const MemoriesPage = lazy(() => import("../features/memories/index.ts").then(m => ({ default: m.MemoriesPage })));
const ActivityPage = lazy(() => import("../features/activity/index.ts").then(m => ({ default: m.ActivityPage })));
const ProfilePage = lazy(() => import("../features/profile/index.ts").then(m => ({ default: m.ProfilePage })));
const PhoneApp = lazy(() => import("../features/phone/index.ts").then(m => ({ default: m.PhoneApp })));
const PlaytestPage = lazy(() => import("../features/playtest/index.ts").then(m => ({ default: m.PlaytestPage })));
const SettingsPage = lazy(() => import("../features/settings/index.ts").then(m => ({ default: m.SettingsPage })));
const StyleguidePage = lazy(() => import("../features/styleguide/index.ts").then(m => ({ default: m.StyleguidePage })));
const HomePage = lazy(() => import("../features/home/index.ts").then(m => ({ default: m.HomePage })));
const PlayPage = lazy(() => import("../features/play/index.ts").then(m => ({ default: m.PlayPage })));
const LiveGamePage = lazy(() => import("../features/live-game/index.ts").then(m => ({ default: m.LiveGamePage })));
const GamePreviewPage = lazy(() => import("../features/game/index.ts").then(m => ({ default: m.GamePreviewPage })));

/** The one place a route becomes a screen. A new page is a line here and a
 *  line in routes.ts. */
function screen(route: Route): ReactNode {
  switch (route.page) {
    case "home": return <HomePage />;
    case "play": return <PlayPage deckId={route.deckId} />;
    case "live-game": return <LiveGamePage gameId={route.gameId} key={route.gameId} />;
    case "game-preview": return <GamePreviewPage deckId={route.deckId} />;
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
      return <InboxPage entryKey={route.entry} />;
    case "memories":
      return <MemoriesPage />;
    case "profile":
      return <ProfilePage />;
    case "activity":
      return <ActivityPage />;
    case "settings":
      return <SettingsPage />;
    case "design":
      return <StyleguidePage />;
    case "shared":
    case "user":
    case "missing":
      return <Page><EmptyState heading="h1" title="There is nothing at this address">Check the link, or go <a href={href({ page: "decks" })}>back to all decks</a>.</EmptyState></Page>;
  }
}

/** Below 720 px the signed-in app is the read-and-rule phone view (D17). */
export function SignedIn({ route }: { route: Route }) {
  const phone = useIsPhone();
  return phone ? <PhoneApp route={route} /> : <AppShell route={route}>{screen(route)}</AppShell>;
}

function DesktopPlaytest({ deckId }: { deckId: number }) {
  const narrow = useMediaQuery(PANEL_QUERY);
  return narrow ? <Page><EmptyState heading="h1" title="Open on a larger screen">The playtester needs a desktop window at least 1100 px wide. <a href={href({ page: "deck", deckId })}>Back to the deck</a></EmptyState></Page> : <PlaytestPage deckId={deckId} />;
}
