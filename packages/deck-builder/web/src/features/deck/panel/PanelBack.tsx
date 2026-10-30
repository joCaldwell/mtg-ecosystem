import type { Ref } from "react";
import { Button, Icon } from "../../../ui/index.ts";
import { useDeckUi } from "../DeckUi.tsx";
import { panelViewLabel } from "../panelViews.ts";
import styles from "./PanelBack.module.css";

/** The card and token panels' way back to the tab they were opened from:
 *  an arrow and a visible "Back" (owner decision, 2026-10-29). */
export function PanelBack({ backRef }: { backRef: Ref<HTMLButtonElement> }) {
  const ui = useDeckUi();
  return (
    <Button ref={backRef} variant="ghost" className={styles.back} aria-label={`Back to ${panelViewLabel(ui.tab)}`} onClick={() => ui.openCardPanel(null)}>
      <Icon name="back" /> Back
    </Button>
  );
}
