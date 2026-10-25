import type { ReactNode } from "react";
import { api } from "../api/client.ts";
import { LiveProvider } from "../api/live.tsx";
import { CardHoverProvider, CardLookupProvider, type CardInfo } from "../mtg/index.ts";
import { ToastProvider } from "../ui/index.ts";
import { SessionProvider } from "./Session.tsx";
import { FocusProvider } from "./Focus.tsx";
import { InboxProvider } from "./Inbox.tsx";

// Exact name resolution, the same rule the server applies everywhere (spec
// §4.3): a name that does not resolve shows nothing rather than a guess.
const lookupCard = (name: string): Promise<CardInfo | null> =>
  api.resolveCard(name).then((r) => r.matches[0] ?? null);

/** Everything app-wide a screen may reach for through context, in one place. */
export function AppProviders({ children }: { children: ReactNode }) {
  return (
    <ToastProvider>
      <SessionProvider><LiveProvider>
        <CardLookupProvider lookup={lookupCard}>
          <CardHoverProvider>
            <InboxProvider>
              <FocusProvider>{children}</FocusProvider>
            </InboxProvider>
          </CardHoverProvider>
        </CardLookupProvider>
      </LiveProvider></SessionProvider>
    </ToastProvider>
  );
}
