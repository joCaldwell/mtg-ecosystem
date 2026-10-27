import type { DeckState } from "../../../api/client.ts";
import { CardRef, ManaCost } from "../../../mtg/index.ts";
import { Eyebrow, KeyValue, Stack, Text } from "../../../ui/index.ts";
import { ManaFaceTable } from "./ManaFaceTable.tsx";
import styles from "./ManaComparison.module.css";

type Mana = DeckState["computed"]["mana"];
const REASONS: Record<string, string> = {
  unsupported_mana_text: "Conditional or unsupported mana text",
  unassigned_land: "Land without a recognized direct colour",
  indirect_text: "Indirect land search",
  quoted_or_granted_text: "Quoted or granted production",
  unbalanced_text: "Unbalanced Oracle text",
  unsupported_layout: "Unsupported layout",
  missing_faces: "Missing face data",
  malformed_faces: "Incomplete face data",
  unsupported_symbol: "Unsupported cost symbol",
  malformed_cost: "Malformed cost",
};
export function ManaComparison({ mana }: { mana: Mana }) {
  const face = mana.first_face, coverage = mana.coverage;
  const flexible = (value: Mana["first_face"]) => Object.entries(value.costs.flexible_symbols).length
    ? <span className={styles.flexible}>{Object.entries(value.costs.flexible_symbols).map(([symbol, count]) => <span key={symbol}><ManaCost cost={symbol} /> × {count}</span>)}</span> : "None";
  const separateSources = (value: Mana["first_face"]) => <KeyValue items={[
    ...(value.sources.land.any_color_copies || value.sources.nonland.any_color_copies ? [["Any-colour sources", `${value.sources.land.any_color_copies} lands · ${value.sources.nonland.any_color_copies} other permanents`] as [string, string]] : []),
    ...(value.costs.fixed_symbols.C || value.sources.land.by_type.C || value.sources.nonland.by_type.C ? [["Colourless C", `${value.costs.fixed_symbols.C} cost symbols · ${value.sources.land.by_type.C} lands · ${value.sources.nonland.by_type.C} other permanents`] as [string, string]] : []),
    ...(Object.keys(value.costs.flexible_symbols).length ? [["Flexible costs", flexible(value)] as [string, ReturnType<typeof flexible>]] : []),
  ]} />;
  const otherCosts = (value: Mana["first_face"]) => <KeyValue items={[
    ["Generic / snow", `${value.costs.generic_units} generic units · ${value.costs.snow_symbols} snow symbols`],
    ["Variable symbols", Object.entries(value.costs.variable_symbols).filter(([, count]) => count > 0).map(([symbol, count]) => `${symbol} × ${count}`).join(" · ") || "None"],
    ["No printed cost", `${value.costs.no_printed_cost_nonland_copies} nonland copies`],
  ]} />;
  return <section className={styles.section} aria-label="Mana demand and sources">
    <Eyebrow>Mana costs and sources</Eyebrow>
    {coverage.other_face_copies > 0 && <Text tone="muted" size="sm">First printed faces; other faces below.</Text>}
    <ManaFaceTable face={face} />
    <Text tone="muted" size="sm">Direct source copies may overlap; not available mana or casting odds.</Text>
    {separateSources(face)}
    <details className={styles.details}>
      <summary>Other cost symbols</summary>
      {otherCosts(face)}
    </details>
    {coverage.other_face_copies > 0 && <details className={styles.details}>
      <summary>Other printed faces · {coverage.other_face_copies} {coverage.other_face_copies === 1 ? "copy" : "copies"}</summary>
      <Stack gap={4}>
        <Text size="sm" tone="muted">Separate from the first face; not simultaneous resources.</Text>
        <ManaFaceTable face={mana.other_faces} />
        {separateSources(mana.other_faces)}
        {otherCosts(mana.other_faces)}
      </Stack>
    </details>}
    <details className={styles.details}>
      <summary>Source coverage · {coverage.unresolved_source_copies} unresolved{coverage.partial_source_copies ? ` · ${coverage.partial_source_copies} partial` : ""}{coverage.cost_issue_copies ? ` · ${coverage.cost_issue_copies} cost issues` : ""}</summary>
      <Stack gap={4}>
      <Text size="sm" tone="soft">Across printed faces: {coverage.recognized_source_copies} source copies recognized ({coverage.partial_source_copies} partial) · {coverage.no_supported_source_pattern_copies} without a supported pattern.</Text>
      <Text size="sm" tone="muted">Conditions, spells, fetches and granted production need review. No supported pattern means unrecognized, not a confirmed non-producer.</Text>
      <Stack gap={4}>{mana.issues.map((issue, index) => <div key={`${issue.oracle_id}/${issue.face_index}/${index}`} className={styles.issue}>
        <div><CardRef name={issue.name} />{issue.quantity > 1 ? ` × ${issue.quantity}` : ""}</div>
        <Text tone="muted" size="sm">{REASONS[issue.reason] ?? issue.reason}{issue.face_index > 0 ? ` · face ${issue.face_index + 1}` : ""}{issue.excerpt ? `: ${issue.excerpt}` : ""}</Text>
      </div>)}</Stack>
      <Text tone="faint" size="sm">Main deck and commanders only; maybe cards and the companion are excluded.</Text>
      </Stack>
    </details>
  </section>;
}
