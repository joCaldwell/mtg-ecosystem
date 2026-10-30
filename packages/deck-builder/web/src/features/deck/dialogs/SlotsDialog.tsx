import { useCallback, useEffect, useState } from "react";
import { Explainer } from "../../../ui/index.ts";
import { Button, Dialog, Section, Stack, Text } from "../../../ui/index.ts";
import { SaveStatusContext, type SaveStatus } from "./saveStatus.ts";
import { SlotsTable } from "./SlotsTable.tsx";
import { TagsSection } from "./TagsSection.tsx";
import styles from "./SlotsDialog.module.css";

const SAVED_MS = 2000;

// Slots say what a card does in the deck, tags are free labels (spec §5.4).
// The server rejects names that collide with a card type or a search prefix;
// its message is shown as it comes. Edits save on their own, so the footer
// says when one has landed.
export function SlotsDialog({ onClose }: { onClose: () => void }) {
  const [status, setStatus] = useState<SaveStatus>("idle");
  const report = useCallback((next: SaveStatus) => setStatus(next), []);
  useEffect(() => {
    if (status !== "saved") return;
    const timer = setTimeout(() => setStatus("idle"), SAVED_MS);
    return () => clearTimeout(timer);
  }, [status]);

  return (
    <SaveStatusContext.Provider value={report}>
      <Dialog
        title="Slots and tags"
        width="lg"
        onClose={onClose}
        footer={<>
          <Text role="status" tone="muted" size="sm" className={styles.status}>
            {status === "saving" ? "Saving…" : status === "saved" ? "Saved" : ""}
          </Text>
          <Button onClick={onClose}>Close</Button>
        </>}
      >
        <Stack gap={7}>
          <Explainer title="Slots and tags" label="How slot targets work">
            <p>Slots describe purpose; tags track sourcing and status. Both are optional. Only primary slots count toward targets; secondary purposes are counted separately.</p>
            <p>Leave a target blank for no limit. Use the same Min and Max for an exact target.</p>
          </Explainer>
          <Section title="Slots">
            <SlotsTable />
          </Section>
          <Section title="Tags">
            <TagsSection />
          </Section>
        </Stack>
      </Dialog>
    </SaveStatusContext.Provider>
  );
}
