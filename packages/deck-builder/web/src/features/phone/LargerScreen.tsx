// Everything a phone does not do (D17): one screen that says what is there
// and where to do it instead, rather than a squeezed editor.

import { navigate, type Route } from "../../app/routes.ts";
import { Button, Stack, Text } from "../../ui/index.ts";
import styles from "./LargerScreen.module.css";

const PAGES: Partial<Record<Route["page"], { title: string; task: string }>> = {
  playtest: { title: "Playtest", task: "use the playtester" },
  memories: { title: "Memories", task: "read and edit memories" },
  profile: { title: "Profile", task: "edit your profile" },
  activity: { title: "Activity", task: "read the activity feed" },
  settings: { title: "Settings", task: "manage settings, connections and tokens" },
  design: { title: "Design reference", task: "browse the design reference" },
};

export function LargerScreen({ route }: { route: Route }) {
  const page = PAGES[route.page];
  return (
    <div className={styles.page}>
      <Stack gap={5}>
        <Text as="h1" size="xl" weight="bold">{page?.title ?? "Open on a larger screen"}</Text>
        <Text as="p" tone="muted">Open on a larger screen{page ? ` to ${page.task}` : " to use this page"}.</Text>
        <div><Button size="touch" onClick={() => navigate({ page: "decks" })}>Back to decks</Button></div>
      </Stack>
    </div>
  );
}
