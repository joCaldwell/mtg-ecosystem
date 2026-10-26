import { useVoice } from "../../app/Preferences.tsx";
import { useState } from "react";
import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { useInbox } from "../../app/Inbox.tsx";
import { ago } from "../../domain/time.ts";
import { RichText } from "../../mtg/index.ts";
import {
  Button,
  EmptyState,
  ErrorNote,
  Loading,
  Meter,
  Page,
  PageHeader,
  Row,
  Stack,
  Text,
  TextArea,
} from "../../ui/index.ts";
import { DocumentEditCard } from "../decisions/index.ts";
import styles from "./ProfilePage.module.css";

/** The soft cap on the profile's length, spec §6. Mirrors PROFILE_WORD_CAP in the server's documents.ts;
 *  the saved text's own `over_cap` comes from the server, this only draws the bar and judges a draft. */
const SOFT_CAP = 600;

/** Counted the way the server counts: whitespace-separated words of the trimmed text. */
function wordCount(text: string): number {
  const t = text.trim();
  return t ? t.split(/\s+/).length : 0;
}

/** The player profile (spec §6): the owner's document, read by every agent first. Edited directly here;
 *  an agent can only propose a change, which shows above for the owner to rule on. */
export function ProfilePage() {
  const voice = useVoice();
  const profile = useQuery(() => api.getProfile(), []);
  const inbox = useInbox();
  // null means not editing. The draft lives here, not in the query, so a live refetch never overwrites it.
  const [draft, setDraft] = useState<string | null>(null);
  // The saved text when editing began, to notice if it changes underneath the draft.
  const [base, setBase] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const saved = profile.data?.profile;
  const pending = (inbox.inbox?.document_edits ?? []).filter((e) => e.kind === "profile");
  const editing = draft !== null;

  function startEditing() {
    if (!saved) return;
    setBase(saved.body_md);
    setDraft(saved.body_md);
    setError(null);
  }

  async function save() {
    if (draft === null) return;
    setBusy(true);
    setError(null);
    try {
      await api.setProfile(draft);
      setDraft(null);
      profile.reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  const words = editing ? wordCount(draft) : (saved?.word_count ?? 0);
  const over = editing ? words > SOFT_CAP : (saved?.over_cap ?? false);
  const empty = saved != null && saved.body_md.trim() === "";
  const changedUnderneath = editing && saved != null && saved.body_md !== base;

  return (
    <Page
      width="md"
      header={
        <PageHeader
          title="Profile"
          subtitle={voice("Your tastes, your playgroup and the games you want to play, in your words.", "Who you are as a builder, in your words. Every agent reads this first, in every session.")}
          actions={
            !editing && saved ? (
              <Button onClick={startEditing}>{empty ? "Write your profile" : "Edit"}</Button>
            ) : undefined
          }
        />
      }
    >
      {profile.error && !saved && <ErrorNote>{profile.error}</ErrorNote>}
      {profile.loading && <Loading />}

      {pending.map((edit) => (
        <DocumentEditCard
          key={edit.id}
          edit={edit}
          onRuled={(r) => {
            inbox.apply(r.inbox);
            profile.reload();
          }}
        />
      ))}

      {saved && (
        <Stack gap={5}>
          {editing ? (
            <>
              <TextArea
                autosize
                rows={14}
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                aria-label="Profile"
                placeholder={"What do you like to build? What does your playgroup allow?\nWhat will you never play?"}
                className={styles.editor}
                autoFocus
              />
              {changedUnderneath && (
                <Text tone="warn" size="sm">
                  The saved profile changed while you were editing, for example because you accepted an edit above.
                  Saving replaces it with the text in this box.
                </Text>
              )}
            </>
          ) : empty ? (
            <EmptyState title="Your profile is empty">
              {voice("Write about your tastes, your playgroup, your budget, and what you will never play.", "Write what an agent should know before it suggests anything: your tastes, your playgroup and its power level, your budget stance, and what you will never play.")}
            </EmptyState>
          ) : (
            <div className={styles.doc}>
              <RichText text={saved.body_md} />
            </div>
          )}

          <Stack gap={3}>
            <Row gap={5}>
              <Meter
                className={styles.meter}
                value={words}
                max={SOFT_CAP}
                tone={over ? "warn" : "neutral"}
                aria-label="Words against the soft cap"
              />
              <Text mono tone={over ? "warn" : "faint"} size="sm" className={styles.count}>
                {words} of about {SOFT_CAP} words
              </Text>
            </Row>
            {over && (
              <Text tone="warn" size="sm">
                {editing ? "This is" : "Your profile is"} over the soft cap of {SOFT_CAP} words. Agents load all of it
                every session, so a shorter profile leaves them more room for the deck.
              </Text>
            )}
          </Stack>

          {error && <ErrorNote>{error}</ErrorNote>}
          {editing && (
            <Row gap={4} justify="end">
              <Button variant="ghost" onClick={() => setDraft(null)} disabled={busy}>
                Cancel
              </Button>
              <Button variant="primary" onClick={save} disabled={busy || draft === saved.body_md}>
                {busy ? "Saving…" : "Save"}
              </Button>
            </Row>
          )}

          <Text tone="faint" size="sm">
            {!empty && <>Saved {ago(saved.updated_at)}. </>}
            You edit this directly. Agents can only propose changes, which appear above for you to accept or reject.
          </Text>
        </Stack>
      )}
    </Page>
  );
}
