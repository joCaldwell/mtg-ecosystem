import { useRef, useState } from "react";
import { api, type DeckState, type Proposal } from "../../api/client.ts";
import { useInbox } from "../../app/Inbox.tsx";
import { ConfirmDialog, ErrorNote, Stack, Text, useToast } from "../../ui/index.ts";
import { DecisionControls } from "../decisions/index.ts";

/** Decide the whole selected proposal through the existing owner capability (§8.2). */
export function ProposalDecision({ proposal, state, onDecided }: { proposal: Proposal; state: DeckState; onDecided: () => void }) {
  const inbox = useInbox();
  const { toast } = useToast();
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const submitting = useRef(false);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<(() => Promise<void>) | null>(null);
  const current = state.proposals.find(p => p.id === proposal.id);
  if (state.deck.category === "archived") return <Text tone="muted">This archived deck is read-only.</Text>;
  if (!current?.items.some(item => item.status === "pending")) return <Text tone="muted">This proposal is no longer waiting.</Text>;

  function decide(outcome: "accept" | "reject", text = "") {
    // Capture exactly what the owner reviewed, including the revision, before confirmation.
    const revision = state.deck.revision;
    const stale = current!.stale;
    const submit = async () => {
      if (submitting.current) return;
      submitting.current = true;
      setBusy(true);
      setError(null);
      try {
        const result = await api.ruleProposal(proposal.deck_id, proposal.id, outcome, text.trim(), {
          expected_revision: revision,
          ...(outcome === "accept" && stale ? { acknowledge_stale: true } : {}),
        });
        inbox.apply(result.inbox);
        setReason("");
        onDecided();
        toast(outcome === "accept" ? "Accepted the proposal." : "Rejected the proposal.");
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        submitting.current = false;
        setBusy(false);
      }
    };
    if (outcome === "accept" && stale) setConfirm(() => submit);
    else void submit();
  }

  return <Stack gap={4} data-proposal-decision>
    <DecisionControls rejecting={false} busy={busy || confirm !== null}
      reason={{ value: reason, onChange: setReason, label: "Reason for this proposal, optional", placeholder: "Reason, optional" }}
      onAccept={text => decide("accept", text)} onReject={text => decide("reject", text)}
      onStartReject={() => {}} onCancelReject={() => {}} />
    {error && <ErrorNote>{error}</ErrorNote>}
    {confirm && <ConfirmDialog title="Accept older proposal?" confirmLabel="Accept reviewed changes"
      onClose={() => setConfirm(null)} onConfirm={() => { setConfirm(null); void confirm(); }}>
      This proposal was based on r{current.base_revision ?? "unknown"}; the deck is now r{state.deck.revision}. Review the changed list before accepting.
    </ConfirmDialog>}
  </Stack>;
}
