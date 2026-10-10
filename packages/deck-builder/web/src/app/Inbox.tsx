// What is waiting on the owner, across every deck. One fetch shared by the
// sidebar badge and the inbox page; a ruling's response carries the fresh
// inbox, which is applied here instead of refetching.

import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { api, type Inbox } from "../api/client.ts";
import { useQuery } from "../api/useQuery.ts";
import { waitingCount } from "../domain/inbox.ts";

interface InboxStore {
  inbox: Inbox | null;
  /** Proposals with something pending, plus proposed document edits. */
  waiting: number;
  apply: (inbox: Inbox) => void;
  reload: () => void;
}

const InboxContext = createContext<InboxStore>({ inbox: null, waiting: 0, apply: () => {}, reload: () => {} });

export function InboxProvider({ children }: { children: ReactNode }) {
  const query = useQuery(() => api.getInbox(), []);
  const [inbox, setInbox] = useState<Inbox | null>(null);
  useEffect(() => {
    if (query.data) setInbox(query.data);
  }, [query.data]);
  const apply = useCallback((next: Inbox) => setInbox(next), []);
  return (
    <InboxContext.Provider value={{ inbox, waiting: inbox ? waitingCount(inbox) : 0, apply, reload: query.reload }}>
      {children}
    </InboxContext.Provider>
  );
}

export function useInbox(): InboxStore {
  return useContext(InboxContext);
}
