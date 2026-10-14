import type { ReactNode } from "react";
import type { Route } from "./routes.ts";
import { Sidebar } from "./Sidebar.tsx";
import styles from "./AppShell.module.css";

/** The frame every page sits in: the sidebar, and one main region that the
 *  page fills and scrolls inside. */
export function AppShell({ route, children }: { route: Route; children: ReactNode }) {
  return (
    <div className={`${styles.shell} ${route.page === "playtest" ? styles.focused : ""}`}>
      {route.page !== "playtest" && <Sidebar route={route} />}
      <main className={styles.main}>{children}</main>
    </div>
  );
}
