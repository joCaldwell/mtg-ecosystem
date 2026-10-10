import { useDeck } from "./store.tsx";
import { DocumentEditCard, ProposalCard } from "./Rulings.tsx";

/** Everything on this deck that is waiting for a ruling. */
export function ProposalSection() {
  const { state, run } = useDeck();
  const { proposals, document_edits, deck } = state!;
  const empty = document_edits.length === 0 && proposals.length === 0;

  return (
    <div className="proposals-panel">
      {document_edits.map((e) => (
        <DocumentEditCard key={e.id} edit={e} rule={run} />
      ))}
      {proposals.map((p) => (
        <ProposalCard key={p.id} proposal={p} rule={run} />
      ))}
      {empty && (
        <div className="proposal-empty">
          <h3>Nothing waiting</h3>
          <p>
            {deck.status === "draft"
              ? "This deck is a draft, so an agent edits it directly. Make it active to have changes come here for a ruling."
              : "When an agent proposes card changes or an edit to the brief, it appears here for you to accept or reject."}
          </p>
        </div>
      )}
    </div>
  );
}
