import type { Ref } from "react";
import { Icon, IconButton } from "../../../ui/index.ts";
import { useDeckUi } from "../DeckUi.tsx";
import { panelViewLabel } from "../panelViews.ts";
import styles from "./PanelBack.module.css";

/** The card and token panels' way back to the tab they were opened from: an
 *  icon-only arrow with a comfortable square hit area. A visible "Back" label
 *  crowded the header and wrapped card names (owner decision, 2026-10-29); the
 *  hover label and accessible name name the previous view. */
export function PanelBack({ backRef }: { backRef: Ref<HTMLButtonElement> }) {
  const ui = useDeckUi();
  return (
    <IconButton ref={backRef} label={`Back to ${panelViewLabel(ui.tab)}`} className={styles.back} onClick={() => ui.openCardPanel(null)}>
      <Icon name="back" />
    </IconButton>
  );
}
