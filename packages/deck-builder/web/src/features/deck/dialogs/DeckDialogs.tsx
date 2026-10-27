import { useDeckUi } from "../DeckUi.tsx";
import { CompareDialog } from "./CompareDialog.tsx";
import { DeleteDeckDialog } from "./DeleteDeckDialog.tsx";
import { ForkDialog } from "./ForkDialog.tsx";
import { ImportDialog } from "../../import/index.ts";
import { Button, Dialog } from "../../../ui/index.ts";
import { ExportPanel } from "./ExportPanel.tsx";
import { useDeck } from "../DeckStore.tsx";
import { ShareDialog } from "./ShareDialog.tsx";
import { SlotsDialog } from "./SlotsDialog.tsx";

/** The dialog the deck screen has asked for, if any. */
export function DeckDialogs() {
  const { dialog, closeDialog } = useDeckUi();
  const { deckId, apply } = useDeck();
  switch (dialog) {
    case "fork":
      return <ForkDialog onClose={closeDialog} />;
    case "compare":
      return <CompareDialog onClose={closeDialog} />;
    case "import":
      return <ImportDialog deckId={deckId} onApplied={apply} onClose={closeDialog} />;
    case "export":
      return <Dialog title="Export" width="lg" onClose={closeDialog} footer={<Button onClick={closeDialog}>Close</Button>}><ExportPanel /></Dialog>;
    case "slots":
      return <SlotsDialog onClose={closeDialog} />;
    case "share":
      return <ShareDialog onClose={closeDialog} />;
    case "delete":
      return <DeleteDeckDialog onClose={closeDialog} />;
    case null:
      return null;
  }
}
