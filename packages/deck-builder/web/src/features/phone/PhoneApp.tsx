import { href, type Route } from "../../app/routes.ts";
import { EmptyState } from "../../ui/index.ts";
import { LargerScreen } from "./LargerScreen.tsx";
import { PhoneDeck } from "./PhoneDeck.tsx";
import { PhoneDecks } from "./PhoneDecks.tsx";
import { PhoneInbox } from "./PhoneInbox.tsx";
import { PhoneShell } from "./PhoneShell.tsx";
import styles from "./LargerScreen.module.css";

/** The signed-in app below 720 px (spec §11.2): read and rule. Only the
 *  desktop-only screens say "Open on a larger screen"; an address that is
 *  nothing says so, as it does on desktop. */
export function PhoneApp({ route }: { route: Route }) {
  return (
    <PhoneShell route={route}>
      {route.page === "decks" || route.page === "signin" || route.page === "privacy" || route.page === "terms" ? <PhoneDecks />
        : route.page === "deck" ? <PhoneDeck deckId={route.deckId} />
        : route.page === "inbox" ? <PhoneInbox />
        : route.page === "missing" || route.page === "shared" || route.page === "user"
          ? <div className={styles.page}><EmptyState heading="h1" title="There is nothing at this address">Check the link, or go <a href={href({ page: "decks" })}>back to your decks</a>.</EmptyState></div>
          : <LargerScreen route={route} />}
    </PhoneShell>
  );
}
