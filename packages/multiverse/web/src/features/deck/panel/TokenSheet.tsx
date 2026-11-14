// A token on a read-only deck (a shared page or the phone): its details in
// a dialog, a sheet on a phone. A card that makes it opens that card's sheet.

import { TokenDetail } from "../../../mtg/index.ts";
import type { DeckToken } from "../../../domain/tokens.ts";
import { Dialog } from "../../../ui/index.ts";
import { TokenProducers } from "./TokenProducers.tsx";

export function TokenSheet({ token, onClose, sheet = false }: { token: DeckToken; onClose: () => void; sheet?: boolean }) {
  return (
    <Dialog title={token.name} onClose={onClose} placement={sheet ? "sheet" : "center"}>
      <TokenDetail token={token} producers={<TokenProducers token={token} />} />
    </Dialog>
  );
}
