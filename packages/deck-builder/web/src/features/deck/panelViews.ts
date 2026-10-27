/** Deck-level destinations; card inspection temporarily remembers one of these. */
export const PANEL_VIEWS = [
  { id: "brief", label: "Brief" },
  { id: "power", label: "Power" },
  { id: "stats", label: "Stats" },
  { id: "checks", label: "Checks" },
  { id: "review", label: "Review" },
  { id: "memories", label: "Memories" },
  { id: "history", label: "History" },
] as const;
export type PanelView = typeof PANEL_VIEWS[number]["id"];
export const panelViewLabel = (view: PanelView) => PANEL_VIEWS.find(item => item.id === view)!.label;
