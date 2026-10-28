import { useVoice } from "../../app/Preferences.tsx";
import { useState } from "react";
import { api, type ActivityEntry } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { href } from "../../app/routes.ts";
import { groupSessions } from "../../domain/activity.ts";
import { clockTime, dayLabel } from "../../domain/time.ts";
import { InlineText } from "../../mtg/index.ts";
import { Button, Pill, Chip, Dot, EmptyState, ErrorNote, Loading, Text } from "../../ui/index.ts";
import styles from "./ActivityPage.module.css";

// The action strings recordActivity is called with (src/mcp/tools.ts), as words.
const VERB: Record<string, string> = {
  create_deck: "Created",
  fork_deck: "Forked",
  edit_draft: "Edited a draft",
  define_slots: "Set slots",
  file_cards: "Filed",
  shortlist_cards: "Shortlisted",
  remember: "Remembered",
  revise_memory: "Revised",
  mark_rulings_reviewed: "Read decisions", // Historical action code; stored prose is preserved.
  mark_decisions_reviewed: "Read decisions",
  propose_changes: "Proposed",
  propose_brief_edit: "Brief edit",
  propose_profile_edit: "Profile edit",
};

function verbOf(action: string): string {
  return VERB[action] ?? action.replace(/_/g, " ");
}

/**
 * The type of the historical write, not its current decision status. A brief
 * or profile edit is only a proposal on an active deck; on a
 * draft, or for the owner's own profile, the server applies it and says so in
 * the summary — "Proposed …" is the only signal it leaves.
 */
function wasProposal(e: ActivityEntry): boolean {
  if (e.action === "propose_changes") return true;
  if (e.action === "propose_brief_edit" || e.action === "propose_profile_edit") return e.summary.startsWith("Proposed");
  return false;
}


/** Every write an agent made (spec §11): the audit trail for what needed no approval. */
export function ActivityPanel() {
  const voice = useVoice();
  // Keyset paging (§11.3): each earlier page is remembered by the id it
  // started before, so writes between loads never shift rows.
  const [cursors, setCursors] = useState<number[]>([]);
  const [agent, setAgent] = useState<string | null>(null);
  const before = cursors.at(-1);
  const feed = useQuery(() => api.listActivity({ before_id: before, actor: agent ?? undefined }), [before, agent]);
  const choose = (next: string | null) => { setAgent(next); setCursors([]); };

  const entries = feed.data?.entries ?? [];
  const actors = feed.data?.actors ?? [];
  const sessions = groupSessions(entries);

  return (
    <>
      {feed.error && <ErrorNote>{feed.error}</ErrorNote>}
      {feed.loading ? (
        <Loading />
      ) : entries.length === 0 && actors.length === 0 ? (
        <EmptyState title={voice("No connected activity yet", "No agent has written anything on this page yet")}>
          {voice("You can build decks without an agent. To connect one, open", "Connect or manage an agent in")} <a className={styles.link} href={href({ page: "settings" })}>Settings</a>.
        </EmptyState>
      ) : (
        <>
          <div className={styles.filters} role="group" aria-label="Agents">
            <Pill on={agent == null} onClick={() => choose(null)}>All agents</Pill>
            {actors.map((a) => (
              <Pill key={a} on={agent === a} onClick={() => choose(a)}>
                {a}
              </Pill>
            ))}
          </div>
          <div className={styles.sessions}>
            {sessions.map((s) => {
              // groupSessions keeps names only; the link needs the id.
              const deckIds = new Map<string, number>();
              for (const e of s.entries) if (e.deck_name && e.deck_id != null) deckIds.set(e.deck_name, e.deck_id);
              return (
                <div key={s.key} className={styles.session}>
                  <div className={styles.head}>
                    <Dot tone="agent" pulse />
                    <Text tone="agent" weight="bold">{s.actor}</Text>
                    <span className={styles.decks}>
                      {s.deckNames.map((name, i) => {
                        const id = deckIds.get(name);
                        return (
                          <span key={name}>
                            {i > 0 && ", "}
                            {id != null ? <a href={href({ page: "deck", deckId: id })}>{name}</a> : name}
                          </span>
                        );
                      })}
                    </span>
                    <span className={`${styles.when} ${styles.mono}`}>
                      {dayLabel(s.start)} {clockTime(s.start)}
                      {s.end !== s.start && ` – ${clockTime(s.end)}`}
                    </span>
                  </div>
                  {s.entries.map((e) => (
                    <div key={e.id} className={styles.event}>
                      <span className={styles.time}>{clockTime(e.ts)}</span>
                      <span className={styles.verb}>{verbOf(e.action)}</span>
                      <span className={styles.summary}>
                        <InlineText text={e.summary} />
                      </span>
                      {wasProposal(e) ? <Chip tone="agent">proposal</Chip> : <Chip>applied</Chip>}
                    </div>
                  ))}
                </div>
              );
            })}
          </div>
        </>
      )}
      {(cursors.length > 0 || feed.data?.has_more) && <nav aria-label="Activity pages"><Button disabled={cursors.length === 0} onClick={() => setCursors(cursors.slice(0, -1))}>Previous 100</Button> <Text>Page {cursors.length + 1}</Text> <Button disabled={!feed.data?.has_more} onClick={() => setCursors([...cursors, entries[entries.length - 1].id])}>Next 100</Button></nav>}
    </>
  );
}
