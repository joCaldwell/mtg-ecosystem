import { api } from "./api.ts";
import { useLive } from "./live.tsx";
import { ago, useLoaded } from "./lib.ts";
import { CardText } from "./Markdown.tsx";

const LABEL: Record<string, string> = {
  create_deck: "created a deck",
  fork_deck: "forked a deck",
  edit_draft: "edited a draft",
  define_slots: "changed slots",
  file_cards: "filed cards",
  shortlist_cards: "changed a maybe list",
  remember: "wrote a memory",
  revise_memory: "revised a memory",
  mark_rulings_reviewed: "read your rulings",
  propose_changes: "proposed changes",
  propose_brief_edit: "proposed a brief edit",
  propose_profile_edit: "proposed a profile edit",
};

/** Every write an agent made (spec §11): the trail for what needed no approval. */
export function ActivityPage() {
  const live = useLive();
  const [entries, error] = useLoaded(() => api.listActivity(), [live]);
  return (
    <div className="page">
      <header className="deck-list-heading">
        <h1>Activity</h1>
        <span className="muted">Everything agents have written</span>
      </header>
      <p className="muted">
        Agents write memories, file cards and edit drafts without asking. Each of those writes is listed
        here with who made it, so nothing done without approval is done out of sight.
      </p>
      {error && <div className="error-banner">{error}</div>}
      {entries?.map((e) => (
        <div className="log-row" key={e.id}>
          <span className="muted mono">{ago(e.ts)}</span>
          <span className="chip">{e.actor}</span>
          <span className="name">{LABEL[e.action] ?? e.action}</span>
          <span className="reason">
            {e.deck_id != null && (
              <>
                <a href={`#/deck/${e.deck_id}`}>{e.deck_name ?? `deck ${e.deck_id}`}</a>
                {" · "}
              </>
            )}
            <CardText text={e.summary} />
          </span>
        </div>
      ))}
      {entries && !entries.length && <p className="muted">No agent has written anything yet.</p>}
    </div>
  );
}
