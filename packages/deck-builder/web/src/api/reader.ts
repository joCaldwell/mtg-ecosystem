// Public/bootstrap reads do not pull the editor's API methods into a visit.
import { request } from "./transport.ts";
import type { Session, CardRow, PublicDeckState, PublicProfile } from "./client.ts";
export { ApiError } from "./transport.ts";
export const api = {
  session: () => request<Session>("GET", "/api/session"),
  resolveCard: (name: string) => request<{ matches: CardRow[]; suggestions: string[] }>("GET", `/api/resolve?${new URLSearchParams({ name })}`),
  publicDeck: (slug: string) => request<{ deck: PublicDeckState }>("GET", `/api/public/decks/${encodeURIComponent(slug)}`),
  publicProfile: (handle: string) => request<{ profile: PublicProfile }>("GET", `/api/public/users/${encodeURIComponent(handle)}`),
};
