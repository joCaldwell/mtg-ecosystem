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
      <Explainer title="Slots and tags" label="How slot targets work">
        <p>Slots describe purpose; tags track sourcing and status. Both are optional. Only primary slots count toward targets; secondary purposes are counted separately.</p>
        <p>Leave a target blank for no limit. Use the same Min and Max for an exact target.</p>
      </Explainer>
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
