import type { DeckSummary } from "../../api/client.ts";
import { Menu } from "../../ui/index.ts";

export type DeckAction = "rename" | "category" | "delete";

export function DeckActions({ deck, onAction }: { deck: DeckSummary; onAction: (action: DeckAction) => void }) {
  return (
    <Menu
      label="Actions"
      icon="more"
      ariaLabel={`Actions for ${deck.name}`}
      size="sm"
      align="end"
      items={[
        { id: "rename", label: "Rename", onSelect: () => onAction("rename") },
        { id: "category", label: "Change category", onSelect: () => onAction("category") },
        { id: "delete", label: "Delete", tone: "cut", onSelect: () => onAction("delete") },
      ]}
    />
  );
}
