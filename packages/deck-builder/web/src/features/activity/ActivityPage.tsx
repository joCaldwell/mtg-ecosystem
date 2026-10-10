import { useState } from "react";
import { api, type ActivityEntry } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { href } from "../../app/routes.ts";
import { groupSessions } from "../../domain/activity.ts";
import { clockTime, dayLabel } from "../../domain/time.ts";
import { InlineText } from "../../mtg/index.ts";
import { Chip, Dot, EmptyState, ErrorNote, Loading, Page, PageHeader, Text } from "../../ui/index.ts";
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
  mark_rulings_reviewed: "Read rulings",
  propose_changes: "Proposed",
  propose_brief_edit: "Brief edit",
  propose_profile_edit: "Profile edit",
};

function verbOf(action: string): string {
  return VERB[action] ?? action.replace(/_/g, " ");
}

/**
 * Whether the write is still waiting on the owner. A change proposal always
 * is. A brief or profile edit is only a proposal on an active deck; on a
 * draft, or for the owner's own profile, the server applies it and says so in
 * the summary — "Proposed …" is the only signal it leaves.
 */
function waitingOnYou(e: ActivityEntry): boolean {
  if (e.action === "propose_changes") return true;
  if (e.action === "propose_brief_edit" || e.action === "propose_profile_edit") return e.summary.startsWith("Proposed");
  return false;
}

function Pill({ on, onClick, children }: { on: boolean; onClick: () => void; children: string }) {
  return (
    <button type="button" aria-pressed={on} className={`${styles.pill} ${on ? styles.pillOn : ""}`} onClick={onClick}>
      {children}
    </button>
  );
}

/** Every write an agent made (spec §11): the audit trail for what needed no approval. */
export function ActivityPage() {
  const feed = useQuery(() => api.listActivity(), []);
  const [agent, setAgent] = useState<string | null>(null);

  const entries = feed.data ?? [];
  const actors = [...new Set(entries.map((e) => e.actor))];
  const sessions = groupSessions(agent == null ? entries : entries.filter((e) => e.actor === agent));

  return (
    <Page
      width="lg"
      header={
        <PageHeader
          title="Activity"
          subtitle="Every write an agent has made, grouped by session. This is the audit trail for changes that did not need your approval."
        />
      }
    >
      {feed.error && <ErrorNote>{feed.error}</ErrorNote>}
      {feed.loading ? (
        <Loading />
      ) : entries.length === 0 ? (
        <EmptyState title="No agent has written anything yet.">
          Connect one from <a href={href({ page: "settings" })}><Text tone="agent">Settings</Text></a>.
        </EmptyState>
      ) : (
        <>
          <div className={styles.filters} role="group" aria-label="Agent">
            <Pill on={agent == null} onClick={() => setAgent(null)}>All agents</Pill>
            {actors.map((a) => (
              <Pill key={a} on={agent === a} onClick={() => setAgent(a)}>
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
                <section key={s.key} className={styles.session} aria-label={`${s.actor} session`}>
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
                      {waitingOnYou(e) ? <Chip tone="agent">waiting on you</Chip> : <Chip>applied</Chip>}
                    </div>
                  ))}
                </section>
              );
            })}
          </div>
        </>
      )}
    </Page>
  );
}
