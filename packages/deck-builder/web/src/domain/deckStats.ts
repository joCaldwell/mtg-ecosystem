// Presentation over server-computed buckets only. No card parsing or source
// inference belongs here; geometry and displayed shares use their own totals.
import type { DeckState } from "../api/client.ts";

export const STAT_COLOURS = [
  { id: "W", name: "White" }, { id: "U", name: "Blue" },
  { id: "B", name: "Black" }, { id: "R", name: "Red" },
  { id: "G", name: "Green" },
] as const;
type Colour = typeof STAT_COLOURS[number]["id"];
export function shareLabel(count: number, total: number): string {
  if (!total) return "—";
  if (!count) return "0%";
  if (count === total) return "100%";
  const percent = 100 * count / total;
  if (percent < 0.1) return "<0.1%";
  if (percent > 99.9) return ">99.9%";
  return `${Number(percent.toFixed(1))}%`;
}

export function colourDistribution(counts: Record<Colour, number>) {
  const total = STAT_COLOURS.reduce((sum, colour) => sum + counts[colour.id], 0);
  let offset = 0;
  const rows = STAT_COLOURS.map(colour => {
    const count = counts[colour.id], share = total ? count / total : 0;
    const row = { ...colour, count, share, offset, percentage: shareLabel(count, total) };
    offset += share;
    return row;
  });
  return { total, rows };
}

export function manaDistributions(mana: DeckState["computed"]["mana"]) {
  const face = mana.first_face;
  return {
    costs: colourDistribution(face.costs.fixed_symbols),
    sources: colourDistribution(Object.fromEntries(STAT_COLOURS.map(({ id }) =>
      [id, face.sources.land.by_type[id] + face.sources.nonland.by_type[id]],
    )) as Record<Colour, number>),
  };
}
export type ColourDistribution = ReturnType<typeof colourDistribution>;
