import type { DeckCategory } from "../api/client.ts";

export const DECK_CATEGORY_OPTIONS: Array<{ value: DeckCategory; label: string; title: string }> = [
  { value: "paper", label: "Paper", title: "A physical deck. Agents propose card changes for your approval." },
  { value: "theorycrafted", label: "Theorycrafted", title: "An idea. Agents edit cards directly; brief changes require approval." },
  { value: "archived", label: "Archived", title: "Read-only for you and your agents. You can still share, export or fork it." },
];

export const DECK_CATEGORY_LABELS: Record<DeckCategory, string> = {
  paper: "Paper",
  theorycrafted: "Theorycrafted",
  archived: "Archived",
};
