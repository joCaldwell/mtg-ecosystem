import { Suspense, type ReactNode } from "react";
import { Loading } from "../ui/index.ts";
import { useMediaQuery, PANEL_QUERY } from "./breakpoints.ts";
import { TabletBar, TabletScroll } from "./TabletShell.tsx";
import type { Route } from "./routes.ts";
import { usePreferences } from "./Preferences.tsx";
import { Sidebar } from "./Sidebar.tsx";
import styles from "./AppShell.module.css";

/** The frame every page sits in: the sidebar, and one main region that the
 *  page fills. Every child keeps its position whichever layout applies, so
 *  crossing a breakpoint (an iPad rotating) re-styles the frame instead of
 *  remounting the page: drafts, selection, a review and open dialogs stay,
 *  and nothing refetches. Only the drawer's menu bar comes and goes. */
export function AppShell({ route, children }: { route: Route; children: ReactNode }) {
  const { sidebarCollapsed, sidebarBusy, setSidebarCollapsed, loaded } = usePreferences();
  const tablet = useMediaQuery(PANEL_QUERY);
  const collapsed = !tablet && sidebarCollapsed;
  const focused = !tablet && route.page === "playtest";
  return (
    <div className={tablet ? styles.tablet : `${styles.shell} ${collapsed ? styles.collapsed : ""} ${focused ? styles.focused : ""}`}>
      {tablet ? <TabletBar route={route} /> : !focused && <div className={styles.side}><Sidebar route={route} collapsed={collapsed} onToggle={!tablet ? () => setSidebarCollapsed(!sidebarCollapsed) : undefined} toggleDisabled={sidebarBusy || !loaded} /></div>}
      <main id="main-content" tabIndex={-1} aria-label="Deck builder" className={tablet ? styles.tabletMain : styles.main}>
        {/* A lazy page's first visit suspends here, inside the frame, not at the app root. */}
        <Suspense fallback={<Loading />}>{children}</Suspense>
      </main>
      {tablet && <TabletScroll />}
    </div>
  );
}
