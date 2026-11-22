import { useCallback, useState } from "react";
import { Explainer } from "../../../ui/index.ts";
import { Button, Dialog, Section, Stack, Text } from "../../../ui/index.ts";
import { SaveStatusContext, type SaveStatus } from "./saveStatus.ts";
import { SlotsTable } from "./SlotsTable.tsx";
import { TagsSection } from "./TagsSection.tsx";
import styles from "./SlotsDialog.module.css";

// Slots say what a card does in the deck, tags are free labels (spec §5.4).
// The server rejects names that collide with a card type or a search prefix;
// its message is shown as it comes. Edits save on their own, so once one has
// landed the footer keeps saying so until the dialog closes.
export function SlotsDialog({ onClose }: { onClose: () => void }) {
  const [status, setStatus] = useState<SaveStatus>("idle");
  const [failure, setFailure] = useState("");
  const [savedOnce, setSavedOnce] = useState(false);
  const [pending, setPending] = useState(0);
  const report = useCallback((next: SaveStatus, message?: string) => {
    setPending(count => count + (next === "saving" ? 1 : -1));
    setStatus(next);
    setFailure(next === "failed" ? message ?? "" : "");
    if (next === "saved") setSavedOnce(true);
  }, []);

  return (
    <SaveStatusContext.Provider value={report}>
      <Dialog
        title="Slots and tags"
        width="lg"
        onClose={onClose}
        busy={pending > 0}
        footer={<>
          <Text role="status" tone={status === "failed" ? "cut" : "muted"} size="sm" className={styles.status}>
            {pending > 0 ? "Saving…" : status === "failed" ? failure : savedOnce ? "All changes saved" : ""}
          </Text>
          <Button onClick={onClose} disabled={pending > 0}>Close</Button>
        </>}
      >
        <Stack gap={7}>
          <Explainer title="Slots and tags" label="How slot targets work">
            <p>Slots describe purpose; tags track sourcing and status. Both are optional. Only primary slots count toward targets; secondary purposes are counted separately.</p>
            <p>Leave a target blank for no limit. Use the same Min and Max for an exact target.</p>
          </Explainer>
          <Section heading="h3" title="Slots">
            <SlotsTable />
          </Section>
          <Section heading="h3" title="Tags">
            <TagsSection />
          </Section>
        </Stack>
      </Dialog>
    </SaveStatusContext.Provider>
  );
}
