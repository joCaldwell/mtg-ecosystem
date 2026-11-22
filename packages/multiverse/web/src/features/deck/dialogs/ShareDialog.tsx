// Who can read this deck (spec §18.2). Only the owner chooses, here; no
// agent can. The selected control is the visibility state; a link appears
// once the deck has one.

import { useRef, useState } from "react";
import { api } from "../../../api/client.ts";
import { href } from "../../../app/routes.ts";
import { Button, ButtonLink, Checkbox, Dialog, ErrorNote, Eyebrow, Icon, Segmented, Stack, Text, TextField } from "../../../ui/index.ts";
import { useDeckState } from "../DeckStore.tsx";
import { useAttempt } from "./useAttempt.ts";
import styles from "./ShareDialog.module.css";

export function ShareDialog({ onClose }: { onClose: () => void }) {
  const { deck } = useDeckState();
  const { error, busy, mutate } = useAttempt();
  const [copyStatus, setCopyStatus] = useState("");
  const copyVersion = useRef(0);
  const link = deck.slug ? `${location.origin}${href({ page: "shared", slug: deck.slug })}` : null;

  async function copy() {
    if (!link) return;
    const version = ++copyVersion.current;
    let message = "Link copied.";
    try {
      await navigator.clipboard.writeText(link);
    } catch {
      message = "Could not copy the link. Select the link and copy it manually.";
    }
    if (version === copyVersion.current) setCopyStatus(message);
  }

  function updateSharing(patch: Parameters<typeof api.setSharing>[1]) {
    copyVersion.current++;
    setCopyStatus("");
    void mutate(() => api.setSharing(deck.id, patch));
  }

  return (
    <Dialog title="Share this deck" busy={busy} onClose={onClose} footer={<Button onClick={onClose} disabled={busy}>Done</Button>}>
      <Stack gap={6}>
        <Stack gap={3}>
          <Segmented
            label="Who can see this deck"
            disabled={busy}
            value={deck.visibility}
            onChange={(visibility) => { if (visibility !== deck.visibility) updateSharing({ visibility }); }}
            options={[
              { value: "private", label: "Private" },
              { value: "unlisted", label: <><Icon name="link" /> Unlisted</> },
              { value: "public", label: <><Icon name="globe" /> Public</> },
            ]}
          />
        </Stack>

        <Stack gap={3}>
          <Eyebrow as="h3">Link</Eyebrow>
          {link ? (
            <div className={styles.link}>
              <TextField readOnly value={link} aria-label="Link to this deck" onFocus={(e) => e.currentTarget.select()} />
              <Button onClick={() => void copy()} disabled={busy || deck.visibility === "private"}>Copy link</Button>
              <ButtonLink href={href({ page: "shared", slug: deck.slug! })} target="_blank" rel="noreferrer">Preview public page</ButtonLink>
            </div>
          ) : (
            <Text tone="muted" as="p">Choose Unlisted or Public to create a link.</Text>
          )}
          <Text role="status" as="p" size="sm" className={styles.status}>{copyStatus}</Text>
        </Stack>

        <Stack gap={3}>
          <Eyebrow as="h3">Decisions</Eyebrow>
          <Checkbox
            checked={deck.publish_decisions}
            disabled={busy}
            onChange={(publish_decisions) => updateSharing({ publish_decisions })}
            label="Show this deck's decisions log on its page"
          />
        </Stack>
        <Text role="status" tone="muted" as="p" className={styles.status}>{busy ? "Saving sharing settings…" : ""}</Text>
        {error && <ErrorNote>{error}</ErrorNote>}
      </Stack>
    </Dialog>
  );
}
