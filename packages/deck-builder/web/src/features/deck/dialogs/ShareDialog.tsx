// Who can read this deck (spec §18.2). Only the owner chooses, here; no
// agent can. The selected control is the visibility state; a link appears
// once the deck has one.

import { api } from "../../../api/client.ts";
import { href } from "../../../app/routes.ts";
import { Button, ButtonLink, Checkbox, Dialog, ErrorNote, Eyebrow, Icon, Segmented, Stack, Text, TextField, useToast } from "../../../ui/index.ts";
import { useDeckState } from "../DeckStore.tsx";
import { useAttempt } from "./useAttempt.ts";
import styles from "./ShareDialog.module.css";

export function ShareDialog({ onClose }: { onClose: () => void }) {
  const { deck } = useDeckState();
  const { error, busy, mutate } = useAttempt();
  const { toast } = useToast();
  const link = deck.slug ? `${location.origin}${href({ page: "shared", slug: deck.slug })}` : null;

  async function copy() {
    if (!link) return;
    try { await navigator.clipboard.writeText(link); toast("Link copied."); } catch { toast(`Copy this link: ${link}`); }
  }

  return (
    <Dialog title="Share this deck" onClose={onClose} footer={<Button onClick={onClose}>Done</Button>}>
      <Stack gap={6}>
        <Stack gap={3}>
          <Segmented
            label="Who can see this deck"
            disabled={busy}
            value={deck.visibility}
            onChange={(visibility) => visibility !== deck.visibility && void mutate(() => api.setSharing(deck.id, { visibility }))}
            options={[
              { value: "private", label: "Private" },
              { value: "unlisted", label: <><Icon name="link" /> Unlisted</> },
              { value: "public", label: <><Icon name="globe" /> Public</> },
            ]}
          />
        </Stack>

        <Stack gap={3}>
          <Eyebrow>Link</Eyebrow>
          {link ? (
            <div className={styles.link}>
              <TextField readOnly value={link} aria-label="Link to this deck" onFocus={(e) => e.currentTarget.select()} />
              <Button onClick={() => void copy()} disabled={deck.visibility === "private"}>Copy link</Button>
              <ButtonLink href={href({ page: "shared", slug: deck.slug! })} target="_blank" rel="noreferrer">Preview public page</ButtonLink>
            </div>
          ) : (
            <Text tone="muted" as="p">Choose Unlisted or Public to create a link.</Text>
          )}
        </Stack>

        <Stack gap={3}>
          <Eyebrow>Decisions</Eyebrow>
          <Checkbox
            checked={deck.publish_decisions}
            disabled={busy}
            onChange={(publish_decisions) => void mutate(() => api.setSharing(deck.id, { publish_decisions }))}
            label="Show this deck's decisions log on its page"
          />
        </Stack>
        {error && <ErrorNote>{error}</ErrorNote>}
      </Stack>
    </Dialog>
  );
}
