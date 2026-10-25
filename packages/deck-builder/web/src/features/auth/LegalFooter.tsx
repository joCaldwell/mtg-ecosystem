import { Text, Stack } from "../../ui/index.ts";
// Required notice: https://company.wizards.com/en/legal/fancontentpolicy (§2).
export function LegalFooter({ asOf }: { asOf?: string | null }) {
  return <footer><Stack gap={2}>
    <Text size="xs" tone="faint"><a href="/privacy">Privacy</a> · <a href="/terms">Terms</a></Text>
    <Text size="xs" tone="faint">Deck builder is unofficial Fan Content permitted under the Fan Content Policy. Not approved/endorsed by Wizards. Portions of the materials used are property of Wizards of the Coast. ©Wizards of the Coast LLC.</Text>
    <Text size="xs" tone="faint">Prices are daily estimates from Scryfall bulk data{asOf ? `, as of ${asOf}` : "; the as-of date appears with each loaded estimate"}.</Text>
  </Stack></footer>;
}
