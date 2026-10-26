import { useEffect, type ReactNode } from "react";
import { api } from "../api/client.ts";
import { LiveProvider, useLive } from "../api/live.tsx";
import { CardHoverProvider, CardLookupProvider, useCardLookup, type CardInfo } from "../mtg/index.ts";
import { FocusProvider } from "./Focus.tsx";
import { InboxProvider } from "./Inbox.tsx";

// Exact name resolution, the same rule the server applies everywhere (spec
// §4.3): a name that does not resolve shows nothing rather than a guess.
const lookupCard = (name: string): Promise<CardInfo | null> =>
  api.resolveCard(name).then((r) => r.matches[0] ?? null);

/** What every screen may reach for: card lookups and hover cards. Shared
 *  pages get only these (spec §18.5): no event stream, inbox or focus, all of
 *  which need a session and belong to the signed-in app. */
export function CardProviders({ children }: { children: ReactNode }) {
  return (
    <CardLookupProvider lookup={lookupCard}>
      <CardHoverProvider>{children}</CardHoverProvider>
    </CardLookupProvider>
  );
}

/** Everything the signed-in app reaches for through context, in one place. */
export function AppProviders({ children }: { children: ReactNode }) {
  return (
    <LiveProvider>
      <CardProviders>
        <ClearCardCache />
        <InboxProvider>
          <FocusProvider>{children}</FocusProvider>
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
