import { Explainer } from "../../../ui/index.ts";
import { Button, Dialog, Section, Stack } from "../../../ui/index.ts";
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
      <Explainer title="Slots and tags" summary="Slots describe purpose; tags track sourcing and status.">Primary slots count toward targets; secondary slots add purposes and are counted separately. Both are optional. Group by card type separately. Targets are soft: leave a side empty for no limit, or set Min and Max to the same number for an exact target. Tags track sourcing, ownership, assembly and status, with as many as you like per card and no targets. Agents can file cards and manage tags without proposals.</Explainer>
      <Stack gap={7}>
        <Section title="Slots">
          <SlotsTable />
        </Section>
        <Section title="Tags">
          <TagsSection />
        </Section>
      </Stack>
    </Dialog>
  );
}
