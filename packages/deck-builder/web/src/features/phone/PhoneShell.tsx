// The phone shell (spec §11.2, D17): below 720 px the signed-in app reads and
// rules. A top bar instead of the sidebar, and only the deck list, a deck, and
// the inbox; every other screen says to open it on a larger screen.

import type { ReactNode } from "react";
import { useInbox } from "../../app/Inbox.tsx";
import { href, type Route } from "../../app/routes.ts";
import { useWindowScroll } from "../../app/windowScroll.ts";
import { Badge } from "../../ui/index.ts";
import styles from "./PhoneShell.module.css";

export function PhoneShell({ route, children }: { route: Route; children: ReactNode }) {
  const { waiting } = useInbox();
  const decks = route.page === "decks" || route.page === "deck";
  useWindowScroll();
  return (
    <div className={styles.shell}>
      <header className={styles.bar}>
        <a className={styles.brand} href={href({ page: "decks" })} aria-label="Deck Builder, all decks">
          <span className={styles.mark} aria-hidden="true" />
        </a>
        <nav className={styles.nav} aria-label="Sections">
          <a href={href({ page: "decks" })} aria-current={decks ? "page" : undefined}>Decks</a>
          <a href={href({ page: "inbox" })} aria-current={route.page === "inbox" ? "page" : undefined}>
            Inbox{waiting > 0 && <Badge tone="agent">{waiting}</Badge>}
          </a>
        </nav>
      </header>
      <main id="main-content" tabIndex={-1} aria-label="Your decks and inbox" className={styles.main}>{children}</main>
    </div>
  );
}
