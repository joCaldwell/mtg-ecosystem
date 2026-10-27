// Tablet pieces of the frame, 720–1099 px (AppShell composes them). The
// menu bar opens navigation in a drawer throughout this range. AppShell
// keeps <main> and the sidebar where they are and swaps only these, so an
// iPad rotating across 1100 px does not remount the page.
import { useEffect, useState } from "react";
import { Badge, Button, Dialog } from "../ui/index.ts";
import { href, type Route } from "./routes.ts";
import { useInbox } from "./Inbox.tsx";
import { Sidebar } from "./Sidebar.tsx";
import { useWindowScroll } from "./windowScroll.ts";
import styles from "./AppShell.module.css";

/** Mounted for as long as the tablet frame applies: tablets scroll the
 *  window. Preserve the #9 useWindowScroll contract. */
export function TabletScroll() {
  useWindowScroll();
  return null;
}

/** Below 1100 px: the menu bar and its navigation drawer. */
export function TabletBar({ route }: { route: Route }) {
  const { waiting } = useInbox();
  const names: Record<Route["page"], string> = { decks: "All decks", deck: "Deck", playtest: "Playtest", inbox: "Inbox", memories: "Memories", profile: "Profile", activity: "Activity", settings: "Settings", design: "Design reference", signin: "Sign in", privacy: "Privacy", terms: "Terms", shared: "Shared deck", user: "Profile", missing: "Page not found" };
  const [open, setOpen] = useState(false);
  useEffect(() => setOpen(false), [route]);
  // Unmounted when the window widens past the panel breakpoint, so the
  // drawer cannot linger over desktop.
  return <>
    <header className={styles.bar}>
      <Button size="touch" onClick={() => setOpen(true)} aria-haspopup="dialog">Menu</Button>
      <span className={styles.location}>{names[route.page]}</span>
      <a className={styles.inboxLink} href={href({ page: "inbox" })} aria-current={route.page === "inbox" ? "page" : undefined}>Inbox <Badge className={styles.inboxBadge}>{waiting}</Badge><span className="visually-hidden"> waiting</span></a>
    </header>
    {open && <Dialog title="Navigation" placement="drawer" onClose={() => setOpen(false)}><Sidebar route={route} brand={false} /></Dialog>}
  </>;
}
