import { Explainer } from "../../../ui/index.ts";
import { useVoice } from "../../../app/Preferences.tsx";
import { Button, Dialog, Section, Stack, Text } from "../../../ui/index.ts";
import { SlotsTable } from "./SlotsTable.tsx";
import { TagsSection } from "./TagsSection.tsx";

// Slots say what a card does in the deck, tags are free labels (spec §5.4).
// The server rejects names that collide with a card type or a search prefix;
// its message is shown as it comes.
export function SlotsDialog({ onClose }: { onClose: () => void }) {
  const voice = useVoice();
  return (
    <Dialog
      title="Slots and tags"
      width="lg"
      onClose={onClose}
      footer={<Button onClick={onClose}>Close</Button>}
    >
      <Explainer title="Slots and tags" summary="Slots describe purpose; tags track sourcing and status.">A primary slot counts toward its target. Advanced controls let you add secondary purposes without changing those targets. Slots are optional, and card types are a separate way to group your list.</Explainer>
      <Stack gap={7}>
        <Section title="Slots">
          <Text tone="muted" as="p">
            {voice("Slots describe what a card does. Tags track where it comes from or its status.", "Slots describe purpose. Agents can file cards and manage tags without proposals.")}
            Targets count primary assignments only. Secondary purposes are shown separately and do not affect target checks.
            Targets are soft. Leave a side empty for no limit, or use the same number in both for an exact target.
          </Text>
          <SlotsTable />
        </Section>
        <Section title="Tags">
          <Text tone="muted" as="p">
            Tags track sourcing, ownership, assembly and status. A card can have as many as you like.
            Use secondary slots for additional purposes. Tags carry no targets.
          </Text>
          <TagsSection />
        </Section>
      </Stack>
    </Dialog>
  );
}
