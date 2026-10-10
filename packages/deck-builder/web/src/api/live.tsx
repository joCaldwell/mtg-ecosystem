// Live updates (spec §11). The server's event stream says only "something
// changed"; every surface that shows server state lists `live` as an effect
// dependency and refetches. That is what makes a proposal an agent made in a
// terminal appear here without a reload.

import { createContext, useContext, useEffect, useState, type ReactNode } from "react";

const LiveContext = createContext(0);

export function LiveProvider({ children }: { children: ReactNode }) {
  const [version, setVersion] = useState(0);
  useEffect(() => {
    const source = new EventSource("/api/events");
    // The first message is the stream opening, and a reconnect after the
    // server restarts is one too: both are a reason to refetch.
    source.onmessage = () => setVersion((v) => v + 1);
    return () => source.close();
  }, []);
  return <LiveContext.Provider value={version}>{children}</LiveContext.Provider>;
}

/** A number that changes whenever anything on the server does. */
export function useLive(): number {
  return useContext(LiveContext);
}
