import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { api, type Session } from "../api/client.ts";
import { useToast, ErrorNote, Loading } from "../ui/index.ts";
import { SignInPage, HandlePicker, LegalPage } from "../features/auth/index.ts";

const Context = createContext<{ session: Session; reload: () => void } | null>(null);
/** `open`: a shared page (spec §18.5), which renders signed in or not. */
export function SessionProvider({ children, open = false }: { children: ReactNode; open?: boolean }) {
  const [session,setSession] = useState<Session | null>(null), [error,setError] = useState<string | null>(null);
  const { toast } = useToast();
  const reload = () => { api.session().then(setSession).catch(e => setError(e.message)); };
  useEffect(reload,[]);
  useEffect(() => {
    const failed = (e: Event) => {
      const { status, retryAfter, message } = (e as CustomEvent).detail;
      // A shared page never needs a session; nothing there should send a reader to sign in.
      if (status === 401 && session?.mode === "hosted" && location.pathname !== "/signin" && !open) location.assign("/signin");
      if (status === 403) toast(message || "This action is read-only for your account.");
      if (status === 429) toast(`Slow down. Try again in ${retryAfter} seconds.`);
    };
    window.addEventListener("api-error",failed); return () => window.removeEventListener("api-error",failed);
  },[session?.mode,toast,open]);
  if (!session) return error ? <ErrorNote>{error}</ErrorNote> : <Loading />;
  const path = location.pathname;
  return <Context.Provider value={{session,reload}}>
    {session.mode === "hosted" && (path === "/privacy" || path === "/terms") ? <LegalPage page={path.slice(1) as "privacy" | "terms"} /> :
      open && !session.user?.needs_handle ? children :
      session.mode === "hosted" && !session.user ? <SignInPage /> :
      session.mode === "hosted" && session.user?.needs_handle ? <HandlePicker /> : children}
  </Context.Provider>;
}
export function useSession() {
  const value = useContext(Context);
  if (!value) throw new Error("useSession needs SessionProvider");
  return value;
}
