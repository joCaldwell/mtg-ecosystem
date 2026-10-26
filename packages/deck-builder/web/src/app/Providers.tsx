import { CardProviders } from "./CardProviders.tsx";
import { PreferencesProvider } from "./Preferences.tsx";
import { useEffect, type ReactNode } from "react";
import { LiveProvider, useLive } from "../api/live.tsx";
import { useCardLookup } from "../mtg/index.ts";
import { FocusProvider } from "./Focus.tsx";
import { InboxProvider } from "./Inbox.tsx";

/** Everything the signed-in app reaches for through context, in one place. */
export function AppProviders({ children }: { children: ReactNode }) {
  return (
    <LiveProvider>
      <CardProviders>
        <ClearCardCache />
        <InboxProvider>
          <PreferencesProvider><FocusProvider>{children}</FocusProvider></PreferencesProvider>
        </InboxProvider>
      </CardProviders>
    </LiveProvider>
  );
}

/** A change (a new price source, a refresh) can change what a looked-up card shows (§4.4). */
function ClearCardCache() {
  const live = useLive();
  const { clear } = useCardLookup();
  useEffect(clear, [live, clear]);
  return null;
}
