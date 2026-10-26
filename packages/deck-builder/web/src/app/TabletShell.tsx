// Tablets scroll the window; preserve the #9 useWindowScroll contract.
import { useEffect, useState, type ReactNode } from "react";
import { Button, Dialog } from "../ui/index.ts";
import { href, type Route } from "./routes.ts";
import { Sidebar } from "./Sidebar.tsx";
import { useWindowScroll } from "./windowScroll.ts";
import styles from "./AppShell.module.css";

export function TabletShell({ route, children, drawer }: { route: Route; children: ReactNode; drawer: boolean }) {
  useWindowScroll();
  const [open, setOpen] = useState(false);
  useEffect(() => setOpen(false), [route]);
  return <div className={drawer ? styles.tablet : styles.tabletRail}>
    {drawer ? <header className={styles.bar}>
      <Button size="touch" onClick={() => setOpen(true)} aria-haspopup="dialog">Menu</Button>
      <a href={href({ page: "decks" })}>Deck Builder</a>
    </header> : <div className={styles.rail}><Sidebar route={route} /></div>}
    <main id="main-content" tabIndex={-1} aria-label="Deck builder" className={styles.tabletMain}>{children}</main>
    {open && <Dialog title="Navigation" placement="drawer" onClose={() => setOpen(false)}><Sidebar route={route} /></Dialog>}
  </div>;
}
