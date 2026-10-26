// Live updates (spec §11). The server's event stream says only "something
// changed"; every surface that shows server state lists `live` as an effect
// dependency and refetches. That is what makes a proposal an agent made in a
// terminal appear here without a reload.

import type { Change } from "../../../src/app.ts";
import { createContext, useContext, useEffect, useState, type ReactNode } from "react";

const LiveContext = createContext(0);

export function LiveProvider({ children }: { children: ReactNode }) {
  const [version, setVersion] = useState(0);
  useEffect(() => {
    const source = new EventSource("/api/events");
    // The first message is the stream opening, and a reconnect after the
    // server restarts is one too: both are a reason to refetch. The first
    // one duplicates every query mounted with the stream, but skipping it is
    // not safe: the stream and those queries travel on separate requests, so
    // a query can be read before the stream is registered and miss a write
    // made in between. Removing the duplicate needs the server to say which
    // reads predate the subscription (a change sequence on both).
    source.onmessage = (event) => {
      const change = JSON.parse(event.data) as Change;
      // Tenant-scoped payload names what changed; initially refetch all queries.
      if (change.kind) setVersion((v) => v + 1);
    };
    return () => source.close();
  }, []);
  return <LiveContext.Provider value={version}>{children}</LiveContext.Provider>;
}

/** A number that changes whenever anything on the server does. */
export function useLive(): number {
  return useContext(LiveContext);
}
