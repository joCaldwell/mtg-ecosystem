import { useDeckUi } from "../DeckUi.tsx";
import { CompareDialog } from "./CompareDialog.tsx";
import { DeleteDeckDialog } from "./DeleteDeckDialog.tsx";
import { ForkDialog } from "./ForkDialog.tsx";
import { InteropDialog } from "./InteropDialog.tsx";
import { ShareDialog } from "./ShareDialog.tsx";
import { SlotsDialog } from "./SlotsDialog.tsx";

/** The dialog the deck screen has asked for, if any. */
export function DeckDialogs() {
  const { dialog, closeDialog } = useDeckUi();
  switch (dialog) {
    case "fork":
      return <ForkDialog onClose={closeDialog} />;
    case "compare":
      return <CompareDialog onClose={closeDialog} />;
    case "import":
      return <InteropDialog initialTab="import" onClose={closeDialog} />;
    case "interop":
      return <InteropDialog onClose={closeDialog} />;
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
