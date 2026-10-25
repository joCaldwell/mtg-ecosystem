import type { Route } from "../../app/routes.ts";
import { LargerScreen } from "./LargerScreen.tsx";
import { PhoneDeck } from "./PhoneDeck.tsx";
import { PhoneDecks } from "./PhoneDecks.tsx";
import { PhoneInbox } from "./PhoneInbox.tsx";
import { PhoneShell } from "./PhoneShell.tsx";

/** The signed-in app below 720 px (spec §11.2): read and rule, nothing else. */
export function PhoneApp({ route }: { route: Route }) {
  return (
    <PhoneShell route={route}>
      {route.page === "decks" ? <PhoneDecks />
        : route.page === "deck" ? <PhoneDeck deckId={route.deckId} />
        : route.page === "inbox" ? <PhoneInbox />
        : <LargerScreen route={route} />}
    </PhoneShell>
  );
}
