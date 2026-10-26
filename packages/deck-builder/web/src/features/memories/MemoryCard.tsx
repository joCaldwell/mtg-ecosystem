import { usePreferences, useVoice } from "../../app/Preferences.tsx";
import { useState } from "react";
import { api, type Memory, type MemoryRevision } from "../../api/client.ts";
import { ago } from "../../domain/time.ts";
import { CardRef, RichText } from "../../mtg/index.ts";
import { Box, Button, Chip, ErrorNote, Text, TextArea } from "../../ui/index.ts";
import styles from "./MemoryCard.module.css";

/** The names a memory's text already shows as [[references]], folded for comparison. */
function namedInText(text: string): Set<string> {
  const named = new Set<string>();
  for (const m of text.matchAll(/\[\[([^\]]+)\]\]/g)) named.add(m[1].trim().toLowerCase());
  return named;
}

/**
 * One memory, as the owner sees it (spec §7.5): what it says, where it came
 * from, and what the owner can do. Agents write memories without asking, so
 * nothing is hidden here. Editing one makes it the owner's, after which no
 * agent can change it (§7.3).
 */
export function MemoryCard({
  memory,
  onChanged,
  showScope = true,
}: {
  memory: Memory;
  onChanged: () => void;
  showScope?: boolean;
}) {
  const voice = useVoice();
  const { advanced } = usePreferences();
  const [draft, setDraft] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // The server writes the owner as "owner"; the v1 migration left "owner (from v1)".
  const byOwner = memory.author_via === "ui";
  const active = memory.status === "active";
  const inferred = memory.basis === "inferred";
  const named = namedInText(memory.text);
  const unnamed = memory.cards.filter((c) => !named.has(c.name.toLowerCase()));

  async function revise(revision: MemoryRevision) {
    setError(null);
    setBusy(true);
    try {
      await api.reviseMemory(memory.id, revision);
      setDraft(null);
      onChanged();
    } catch (e) {
      // The server lints [[Card]] references and refuses with suggestions in the message.
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Box dashed={inferred} padding="sm" className={`${styles.card} ${active ? "" : styles.off}`}>
      {draft == null ? (
        <RichText text={memory.text} className={styles.text} />
      ) : (
        <div className={styles.editor}>
          <TextArea
            autosize
            autoFocus
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            aria-label={`Text of memory ${memory.id}`}
          />
          <span className={styles.hint}>{voice("Save your correction to keep this memory in your own words.", "Editing a memory makes it yours. Agents can no longer change it.")}</span>
        </div>
      )}

      {unnamed.length > 0 && draft == null && (
        <div className={styles.cards}>
          {unnamed.map((c) => (
            <CardRef key={c.oracle_id} name={c.name} />
          ))}
        </div>
      )}

      {error && <ErrorNote>{error}</ErrorNote>}

      <div className={styles.meta}>
        {showScope && <Chip>{memory.scope === "global" ? "Global" : (memory.deck_name ?? "Deck")}</Chip>}
        {inferred ? (
          <Chip tone="agent" dashed title="An agent's conclusion. A hypothesis until you confirm it.">
            Inferred · a hypothesis
          </Chip>
        ) : (
          <Chip title="Something you said">Stated</Chip>
        )}
        {memory.status === "superseded" && (
          <Chip>
            Superseded
            {memory.superseded_by != null && <> by memory {memory.superseded_by}</>}
          </Chip>
        )}
        {memory.status === "retired" && <Chip>Retired</Chip>}
        <span>
          {byOwner ? <span title={memory.author_display ?? memory.author}>you</span> : <Text tone="agent">{memory.author_display ?? memory.author}</Text>}
        </span>
        {memory.source_log_id != null && <span>from decision {memory.source_log_id}</span>}
        {advanced && memory.deck_revision != null && <span className={styles.mono}>at r{memory.deck_revision}</span>}
        <span>{ago(memory.revised_at)}</span>
        {memory.owner_touched && !byOwner && <span>edited by you</span>}

        {!memory.read_only && <span className={styles.acts}>
          {draft == null ? (
            <>
              {active && inferred && (
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  title="This is true: mark it as stated"
                  onClick={() => revise({ action: "confirm" })}
                >
                  Confirm
                </Button>
              )}
              {active && (
                <Button size="sm" variant="ghost" disabled={busy} onClick={() => setDraft(memory.text)}>
                  Edit
                </Button>
              )}
              {active ? (
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  title="Take it out of use. It is kept, not deleted."
                  onClick={() => revise({ action: "retire" })}
                >
                  Retire
                </Button>
              ) : (
                <Button size="sm" variant="ghost" disabled={busy} onClick={() => revise({ action: "restore" })}>
                  Restore
                </Button>
              )}
            </>
          ) : (
            <>
              <Button
                size="sm"
                variant="primary"
                disabled={busy || !draft.trim()}
                onClick={() => revise({ action: "update", text: draft })}
              >
                Save
              </Button>
              <Button
                size="sm"
                variant="ghost"
                disabled={busy}
                onClick={() => {
                  setDraft(null);
                  setError(null);
                }}
              >
                Cancel
              </Button>
            </>
          )}
        </span>}
      </div>
    </Box>
  );
}
