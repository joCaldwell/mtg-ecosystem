import type { DeckState } from "../../../api/client.ts";
import { CardRef, ManaCost } from "../../../mtg/index.ts";
import { Eyebrow, KeyValue, Stack, Text } from "../../../ui/index.ts";
import { ManaFaceTable } from "./ManaFaceTable.tsx";
import styles from "./ManaComparison.module.css";

type Mana = DeckState["computed"]["mana"];
const REASONS: Record<string, string> = {
  unsupported_mana_text: "Conditional or unsupported mana text",
  unassigned_land: "Land without a recognized direct colour",
  indirect_text: "Indirect land or token text",
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
  const alternatives = coverage.other_face_copies > 0;
  const faces = [{ label: "First printed face", value: face }, ...(alternatives ? [{ label: "Other printed faces · alternatives", value: mana.other_faces }] : [])];
  return <section className={styles.section} aria-label="Mana demand and sources">
    <Eyebrow>Mana costs and sources</Eyebrow>
    <Text tone="muted" size="sm">Fixed cost symbols and recognized source copies; any-colour output and flexible costs are separate. Not available mana or casting odds.</Text>
    {faces.map(({ label, value }, index) => <div key={label} className={styles.face}>
      <Text weight="medium" size="sm">{label}</Text>
      <ManaFaceTable face={value} />
      <KeyValue items={[
        ["Any-colour sources", `${value.sources.land.any_color_copies} lands · ${value.sources.nonland.any_color_copies} other permanents`],
        ["Colourless C", `${value.costs.fixed_symbols.C} fixed cost symbols · ${value.sources.land.by_type.C} land sources · ${value.sources.nonland.by_type.C} other sources`],
        ["Generic / snow", `${value.costs.generic_units} generic units · ${value.costs.snow_symbols} snow symbols`],
        ["Variable costs", Object.entries(value.costs.variable_symbols).filter(([, count]) => count > 0).map(([symbol, count]) => `${symbol} × ${count}`).join(" · ") || "None"],
        ["Flexible costs", Object.entries(value.costs.flexible_symbols).length ? <span className={styles.flexible}>{Object.entries(value.costs.flexible_symbols).map(([symbol, count]) => <span key={symbol}><ManaCost cost={symbol} /> × {count}</span>)}</span> : "None"],
        ...(value.costs.no_printed_cost_nonland_copies ? [["No printed cost", `${value.costs.no_printed_cost_nonland_copies} nonland copies`] as [string, string]] : []),
      ]} />
      {index > 0 && <Text size="sm" tone="muted">Other faces are alternatives, not simultaneous resources. Their symbols are never added to the first face.</Text>}
    </div>)}
    <Text size="sm" tone="muted">Direct tap patterns only; multicolour source rows overlap. Conditions, spells, fetches and granted production need review.</Text>
    <Text size="sm" tone="soft">{coverage.recognized_source_copies} source copies recognized ({coverage.partial_source_copies} partial) · {coverage.unresolved_source_copies} unresolved · {coverage.no_supported_source_pattern_copies} without a supported pattern.</Text>
    {mana.issues.length > 0 && <details className={styles.details}>
      <summary>Review unresolved mana data{coverage.cost_issue_copies ? ` · ${coverage.cost_issue_copies} copies with cost issues` : ""}</summary>
      <Stack gap={4}>{mana.issues.map((issue, index) => <div key={`${issue.oracle_id}/${issue.face_index}/${index}`} className={styles.issue}>
        <div><CardRef name={issue.name} />{issue.quantity > 1 ? ` × ${issue.quantity}` : ""}</div>
        <Text tone="muted" size="sm">{REASONS[issue.reason] ?? issue.reason}{issue.face_index > 0 ? ` · face ${issue.face_index + 1}` : ""}{issue.excerpt ? `: ${issue.excerpt}` : ""}</Text>
      </div>)}</Stack>
    </details>}
    <Text tone="faint" size="sm">No supported pattern means unrecognized, not a confirmed non-producer. Maybe cards and the companion are excluded.</Text>
  </section>;
}
