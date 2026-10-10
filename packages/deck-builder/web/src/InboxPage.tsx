import { useState } from "react";
import { api, type Inbox } from "./api.ts";
import { useLive } from "./live.tsx";
import { useLoaded } from "./lib.ts";
import { DocumentEditCard, ProposalCard, type Rule } from "./Rulings.tsx";

/** Everything awaiting a ruling, across every deck and the profile (spec §11). */
export function InboxPage() {
  const live = useLive();
  const [fetched, loadError, reload] = useLoaded(() => api.getInbox(), [live]);
  const [error, setError] = useState<string | null>(null);
  const inbox: Inbox | null = fetched;

  const rule: Rule = async (fn) => {
    setError(null);
    try {
      const result = await fn();
      reload();
      return result;
    } catch (e: any) {
      setError(e.message);
      return null;
    }
  };

  const empty = inbox && !inbox.proposals.length && !inbox.document_edits.length;
  return (
    <div className="page">
      <header className="deck-list-heading">
        <h1>Inbox</h1>
        <span className="muted">What agents have proposed and you have not ruled on</span>
      </header>
      {(error ?? loadError) && <div className="error-banner">{error ?? loadError}</div>}
      {inbox && inbox.unreviewed > 0 && (
        <p className="muted">
          {inbox.unreviewed} of your recent rulings and changes have not been read by an agent yet. The
          next agent session will pick them up.
        </p>
      )}
      <div className="proposals-panel">
        {inbox?.document_edits.map((e) => (
          <DocumentEditCard key={e.id} edit={e} rule={rule} showDeck />
        ))}
        {inbox?.proposals.map((p) => (
          <ProposalCard key={p.id} proposal={p} rule={rule} showDeck />
        ))}
        {empty && (
          <div className="proposal-empty">
            <h3>Nothing waiting</h3>
            <p>
              Agents change an active deck's cards, and your brief and profile, only by proposing. Their
              proposals arrive here.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
