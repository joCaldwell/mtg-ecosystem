import type { DeckCategory } from "../api/client.ts";

export const DECK_CATEGORY_OPTIONS: Array<{ value: DeckCategory; label: string; title: string }> = [
  { value: "paper", label: "Paper", title: "A physical deck. Agents propose card changes for your approval." },
  { value: "theorycrafted", label: "Theorycrafted", title: "An idea. Agents edit cards directly; brief changes require approval." },
  { value: "legacy", label: "Legacy", title: "A read-only archive. Restore or fork it to resume editing." },
];

export const DECK_CATEGORY_LABELS: Record<DeckCategory, string> = {
  paper: "Paper",
  theorycrafted: "Theorycrafted",
  legacy: "Legacy",
};
