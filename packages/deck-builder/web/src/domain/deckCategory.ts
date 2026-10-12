import type { DeckCategory } from "../api/client.ts";

export const DECK_CATEGORY_OPTIONS: Array<{ value: DeckCategory; label: string; title: string }> = [
  { value: "paper", label: "Paper", title: "A deck you keep in paper." },
  { value: "theorycrafted", label: "Theorycrafted", title: "A deck idea you are building or exploring." },
  { value: "legacy", label: "Legacy", title: "A past deck you keep for reference." },
];

export const DECK_CATEGORY_LABELS: Record<DeckCategory, string> = {
  paper: "Paper",
  theorycrafted: "Theorycrafted",
  legacy: "Legacy",
};
