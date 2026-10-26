// Tablet pieces of the frame, 720–1099 px (AppShell composes them). From 960
// the sidebar is a rail; below it, a menu bar opens it in a drawer. AppShell
// keeps <main> and the sidebar where they are and swaps only these, so an
// iPad rotating across 1100 px does not remount the page.
import { useEffect, useState } from "react";
import { Button, Dialog } from "../ui/index.ts";
import { href, type Route } from "./routes.ts";
import { Sidebar } from "./Sidebar.tsx";
import { useWindowScroll } from "./windowScroll.ts";
import styles from "./AppShell.module.css";

/** Mounted for as long as the tablet frame applies: tablets scroll the
 *  window. Preserve the #9 useWindowScroll contract. */
export function TabletScroll() {
  useWindowScroll();
  return null;
}

/** Below 960 px: the menu bar and its navigation drawer. */
export function TabletBar({ route }: { route: Route }) {
  const [open, setOpen] = useState(false);
  useEffect(() => setOpen(false), [route]);
  // Unmounted when the window widens past the drawer breakpoint, so the
  // drawer cannot linger over the rail.
  return <>
    <header className={styles.bar}>
      <Button size="touch" onClick={() => setOpen(true)} aria-haspopup="dialog">Menu</Button>
      <a href={href({ page: "decks" })}>Deck Builder</a>
    </header>
    {open && <Dialog title="Navigation" placement="drawer" onClose={() => setOpen(false)}><Sidebar route={route} /></Dialog>}
  </>;
}
