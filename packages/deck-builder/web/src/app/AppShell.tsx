import { useMediaQuery, DRAWER_QUERY, PANEL_QUERY } from "./breakpoints.ts";
import { TabletShell } from "./TabletShell.tsx";
import type { ReactNode } from "react";
import type { Route } from "./routes.ts";
import { Sidebar } from "./Sidebar.tsx";
import styles from "./AppShell.module.css";

/** The frame every page sits in: the sidebar, and one main region that the
 *  page fills and scrolls inside. */
export function AppShell({ route, children }: { route: Route; children: ReactNode }) {
  const tablet = useMediaQuery(PANEL_QUERY);
  const drawer = useMediaQuery(DRAWER_QUERY);
  if (tablet) return <TabletShell route={route} drawer={drawer}>{children}</TabletShell>;
  return (
    <div className={`${styles.shell} ${route.page === "playtest" ? styles.focused : ""}`}>
      {route.page !== "playtest" && <Sidebar route={route} />}
      <main id="main-content" tabIndex={-1} aria-label="Deck builder" className={styles.main}>{children}</main>
    </div>
  );
}
