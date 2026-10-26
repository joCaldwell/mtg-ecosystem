import type { ReactNode } from "react";
import { api } from "../api/reader.ts";
import { CardHoverProvider, CardLookupProvider, type CardInfo } from "../mtg/index.ts";
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

