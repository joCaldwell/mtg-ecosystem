import { Button, Segmented, Spacer } from "../../ui/index.ts";
import type { GroupBy } from "../../domain/grouping.ts";
import { AddBar } from "./AddBar.tsx";
import { useDeckUi } from "./DeckUi.tsx";
import styles from "./DeckToolbar.module.css";

const GROUP_OPTIONS: Array<{ value: GroupBy; label: string }> = [
  { value: "slot", label: "By slot" },
  { value: "type", label: "By type" },
];

export function DeckToolbar() {
  const ui = useDeckUi();
  return (
    <div className={styles.toolbar}>
      <AddBar />
      <Segmented label="Group the list" options={GROUP_OPTIONS} value={ui.groupBy} onChange={ui.setGroupBy} />
      <Spacer />
      <Button variant="ghost" onClick={() => ui.openDialog("slots")}>
        Edit slots
      </Button>
    </div>
  );
}
