// Who can read this deck (spec §18.2). Only the owner chooses, here; no
// agent can. Each choice says in plain words what it does, and the link
// appears once the deck has one.

import { api, type Visibility } from "../../../api/client.ts";
import { href } from "../../../app/routes.ts";
import { Button, Checkbox, Dialog, ErrorNote, Eyebrow, Icon, Segmented, Stack, Text, TextField, useToast } from "../../../ui/index.ts";
import { useDeckState } from "../DeckStore.tsx";
import { useAttempt } from "./useAttempt.ts";
import styles from "./ShareDialog.module.css";

const CONSEQUENCE: Record<Visibility, string> = {
  private: "Only you can see this deck. Its link, if it had one, shows nothing to anyone else.",
  unlisted: "Anyone with the link can read the list, brief, checks and power, without an account. It is not listed on your profile or in search engines.",
  public: "Anyone can read it, and it is listed on your profile. Search engines may index it.",
};

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
            value={deck.visibility}
            onChange={(visibility) => visibility !== deck.visibility && void mutate(() => api.setSharing(deck.id, { visibility }))}
            options={[
              { value: "private", label: "Private" },
              { value: "unlisted", label: <><Icon name="link" /> Unlisted</> },
              { value: "public", label: <><Icon name="globe" /> Public</> },
            ]}
          />
          <Text tone="soft" as="p" role="status">{CONSEQUENCE[deck.visibility]}</Text>
          <Text size="xs" tone="faint" as="p">
            Viewers never see your memories, pending proposals, ownership notes, activity or your agents' names. Only
            you can change this, and agents never can.
          </Text>
        </Stack>

        <Stack gap={3}>
          <Eyebrow>Link</Eyebrow>
          {link ? (
            <div className={styles.link}>
              <TextField readOnly value={link} aria-label="Link to this deck" onFocus={(e) => e.currentTarget.select()} />
              <Button onClick={() => void copy()} disabled={deck.visibility === "private"}>Copy link</Button>
            </div>
          ) : (
            <Text tone="muted" as="p">Choose Unlisted or Public to make a link. It stays the same from then on.</Text>
          )}
          {link && <a href={href({ page: "shared", slug: deck.slug! })} target="_blank" rel="noreferrer">See what viewers see</a>}
        </Stack>

        <Stack gap={3}>
          <Eyebrow>Decisions</Eyebrow>
          <Checkbox
            checked={deck.publish_decisions}
            disabled={busy}
            onChange={(publish_decisions) => void mutate(() => api.setSharing(deck.id, { publish_decisions }))}
            label="Show this deck's decisions log on its page"
          />
          <Text size="xs" tone="muted" as="p">
            Off by default. When on, each entry in the History tab can be hidden from the page on its own. Agents are shown
            only as “an agent”.
          </Text>
        </Stack>
        {error && <ErrorNote>{error}</ErrorNote>}
      </Stack>
    </Dialog>
  );
}
