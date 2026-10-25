// Everything a phone does not do (D17): one screen that says what is there
// and where to do it instead, rather than a squeezed editor.

import { href, type Route } from "../../app/routes.ts";
import { EmptyState } from "../../ui/index.ts";
import styles from "./LargerScreen.module.css";

const WHAT: Partial<Record<Route["page"], string>> = {
  playtest: "The playtester needs a table: open it on a computer.",
  memories: "Memories are read and edited on a larger screen.",
  profile: "Your profile is edited on a larger screen.",
  activity: "The activity feed is on a larger screen.",
  settings: "Settings, connections and tokens are on a larger screen.",
  design: "The design reference is on a larger screen.",
};

export function LargerScreen({ route }: { route: Route }) {
  return (
    <div className={styles.page}>
      <EmptyState title="Open on a larger screen">
        {WHAT[route.page] ?? "This screen is on a larger screen."} On a phone you can read your decks and rule on what
        your agents propose. To change a deck from here, ask your agent and accept or reject what it proposes.{" "}
        <a href={href({ page: "decks" })}>Back to your decks</a>
      </EmptyState>
    </div>
  );
}
