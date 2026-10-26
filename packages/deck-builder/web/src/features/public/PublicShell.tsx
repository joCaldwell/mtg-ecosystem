// The frame of a shared page (spec §18.5): no sidebar, no inbox. A top bar
// with the app's name and the way into your own decks or to sign in.

import type { ReactNode } from "react";
import { href, signInHref } from "../../app/routes.ts";
import { useSession } from "../../app/Session.tsx";
import { useWindowScroll } from "../../app/windowScroll.ts";
import styles from "./PublicShell.module.css";

export function PublicShell({ children }: { children: ReactNode }) {
  const { session } = useSession();
  const signedIn = session.mode === "local" || !!session.user;
  useWindowScroll();
  return (
    <div className={styles.shell}>
      <header className={styles.bar}>
        <a className={styles.brand} href={href({ page: "decks" })}>
          <span className={styles.mark} aria-hidden="true" />
          Deck Builder
        </a>
        <nav className={styles.nav} aria-label="Account">
          {signedIn
            ? <a href={href({ page: "decks" })}>Your decks</a>
            : <a href={signInHref(location.pathname)}>Sign in</a>}
        </nav>
      </header>
      <main className={styles.main}>{children}</main>
      <footer className={styles.footer}>
        Deck builder is unofficial Fan Content permitted under the Fan Content Policy. Not approved/endorsed by Wizards.
        Portions of the materials used are property of Wizards of the Coast. ©Wizards of the Coast LLC. Card data,
        images and price estimates from Scryfall.
      </footer>
    </div>
  );
}
