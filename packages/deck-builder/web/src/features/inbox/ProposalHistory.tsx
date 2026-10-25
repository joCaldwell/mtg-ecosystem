import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { editTitle } from "../../domain/documents.ts";
import { RichText } from "../../mtg/index.ts";
import { Chip, EmptyState, ErrorNote, Loading, Row, Stack, Text } from "../../ui/index.ts";
import { HistoryEntry } from "./HistoryEntry.tsx";

/** Read-only audit view. Earlier drafts never expose approval controls. */
export function ProposalHistory() {
  const history = useQuery(api.proposalHistory, []);
  if (history.error) return <ErrorNote>{history.error}</ErrorNote>;
  if (!history.data) return <Loading />;
  const { document_edits, proposals } = history.data;
  return <Stack gap={5}>
    <Text tone="muted">Recent proposal history, including earlier drafts. Superseded and withdrawn work cannot be approved.</Text>
    {!document_edits.length && !proposals.length && <EmptyState title="No proposal history yet">Proposals and their earlier drafts will appear here.</EmptyState>}
    {document_edits.map(edit => <HistoryEntry key={`edit-${edit.id}`} title={`${editTitle(edit)} · #${edit.id}`} status={edit.disposition === "current" ? edit.status : edit.disposition}
      subject={edit.deck_name ?? "Profile"} author={edit.author} createdAt={edit.created_at}
      basis={[edit.base_revision != null ? `Based on revision ${edit.base_revision}` : "", edit.group_id ? `Edit group #${edit.group_id}` : "", edit.superseded_by ? `Replaced by #${edit.superseded_by}` : ""].filter(Boolean).join(" · ")}>
      <RichText text={edit.reason} />
      <RichText text={edit.payload.content ?? edit.payload.description ?? edit.payload.engine_name ?? ""} />
      {!!edit.piece_names.length && <Text>{edit.piece_names.join(", ")}</Text>}
    </HistoryEntry>)}
    {proposals.map(proposal => <HistoryEntry key={`proposal-${proposal.id}`} title={`Card proposal #${proposal.id}`} status={proposal.disposition === "current" ? proposal.status : proposal.disposition}
      subject={proposal.deck_name} author={proposal.author} createdAt={proposal.created_at}
      basis={`Based on revision ${proposal.base_revision ?? "unknown"}${proposal.superseded_by ? ` · replaced by #${proposal.superseded_by}` : ""}`}>
      <RichText text={proposal.summary} />
      {proposal.items.map(item => <Stack key={item.id} gap={1}><Row wrap><Text>{item.action === "add" ? "Add" : "Cut"} {item.card_name}</Text><Chip>{item.status}</Chip></Row><RichText text={item.reason} /></Stack>)}
    </HistoryEntry>)}
  </Stack>;
}
