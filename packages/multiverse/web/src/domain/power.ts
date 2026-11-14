import type { BracketMapping, Power, PowerProfile } from "../api/client.ts";

export function emptyPowerProfile(): PowerProfile {
  return { intent: null, win_turn: null, early_win_turn: null, consistency: null, resilience: null,
    interaction: null, win_style: null, combos: null, extra_turns: null, mass_land_denial: null };
}
export function bracketLabel(mapping: BracketMapping): string {
  return mapping.bracket == null ? "undetermined" : `B${mapping.bracket}${mapping.provisional ? "?" : ""}`;
}
export function powerRows(power: Power) {
  const t = power.target?.profile;
  const a = power.assessment?.profile;
  const win = (p: PowerProfile | undefined) => p?.win_turn ? `${p.win_turn.min}–${p.win_turn.max}` : "Unknown";
  const early = (p: PowerProfile | undefined) => p?.early_win_turn == null ? "Not recorded" : `Turn ${p.early_win_turn}`;
  const rows = power.fields.map(f => ({
    field: f.label,
    target: t ? f.options.find(o => o.value === t[f.key])?.label ?? "Unknown" : "—",
    current: a ? f.options.find(o => o.value === a[f.key])?.label ?? "Unknown" : "—",
  }));
  rows.splice(1, 0, { field: "Usual win range", target: t ? win(t) : "—", current: a ? win(a) : "—" },
    { field: "Occasional early win", target: t ? early(t) : "—", current: a ? early(a) : "—" });
  const denial = (p: PowerProfile | undefined) => p?.mass_land_denial == null ? "Unknown" : p.mass_land_denial ? "Yes" : "None";
  const gc = power.assessment?.game_changers ?? power.game_changers;
  rows.push({ field: "Mass land denial", target: t ? denial(t) : "—", current: a ? denial(a) : "—" },
    { field: "Game Changers", target: power.target?.game_changers_max == null ? "No limit set" : `≤ ${power.target.game_changers_max}`,
      current: `${gc.count}${gc.unknown_cards ? "+ known" : ""}` });
  return rows;
}
