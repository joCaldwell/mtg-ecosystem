import { Button, Dialog, Section, Stack, Text } from "../../../ui/index.ts";
import { SlotsTable } from "./SlotsTable.tsx";
import { TagsSection } from "./TagsSection.tsx";

// Slots say what a card does in the deck, tags are free labels (spec §5.4).
// The server rejects names that collide with a card type or a search prefix;
// its message is shown as it comes.
export function SlotsDialog({ onClose }: { onClose: () => void }) {
  return (
    <Dialog
      title="Slots and tags"
      width="lg"
      onClose={onClose}
      footer={<Button onClick={onClose}>Close</Button>}
    >
      <Stack gap={7}>
        <Section title="Slots">
          <Text tone="muted" as="p">
            Slots are optional. A card sits in at most one, and targets are soft: a slot outside its target is
            flagged, never blocked. Leave a side empty for no limit, or put the same number in both for exactly
            that many.
          </Text>
          <SlotsTable />
        </Section>
        <Section title="Tags">
          <Text tone="muted" as="p">
            A card can have as many tags as you like. Tags carry no targets.
          </Text>
          <TagsSection />
        </Section>
      </Stack>
    </Dialog>
  );
}
