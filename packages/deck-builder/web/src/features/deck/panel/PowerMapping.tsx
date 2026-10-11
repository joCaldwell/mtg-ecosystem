import type { BracketMapping } from "../../../api/client.ts";
import { Stack, Text } from "../../../ui/index.ts";

export function PowerMapping({ title, mapping }: { title: string; mapping: BracketMapping | null }) {
  return <Stack gap={3}>
    <Text weight="medium">{title}: {mapping?.bracket == null ? "undetermined" : `Bracket ${mapping.bracket} — ${mapping.name}`}</Text>
    {mapping?.provisional && <Text tone="warn" size="sm">Provisional · review the gaps below</Text>}
    {mapping?.reasons.filter(reason => !mapping.conflicts.includes(reason)).map(reason => <Text key={reason} as="p" size="sm" tone="muted">{reason}</Text>)}
    {mapping?.conflicts.map(reason => <Text key={reason} as="p" size="sm" tone="warn">{reason}</Text>)}
    {!!mapping?.unknowns.length && <Text as="p" size="sm" tone="muted">Unknown: {mapping.unknowns.join(", ")}.</Text>}
  </Stack>;
}
