import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { RichText } from "../../mtg/index.ts";
import { ErrorNote, Loading, Stack, Text } from "../../ui/index.ts";

/** Read-only audit view. Earlier drafts never expose approval controls. */
export function ProposalHistory() {
  const history = useQuery(api.proposalHistory, []);
  if (history.error) return <ErrorNote>{history.error}</ErrorNote>;
  if (!history.data) return <Loading />;
  return <Stack gap={3}>
    <Text tone="faint">Recent proposal history, including earlier drafts. Superseded and withdrawn work cannot be approved.</Text>
    {history.data.document_edits.map(edit => <details key={`edit-${edit.id}`}>
      <summary>Document edit #{edit.id} · {edit.deck_name ?? "Profile"} · {edit.kind} · {edit.disposition === "current" ? edit.status : edit.disposition}{edit.group_id ? ` · group #${edit.group_id}` : ""}{edit.superseded_by ? ` → #${edit.superseded_by}` : ""}</summary>
      <Stack gap={2}>
        <Text tone="faint">{edit.author} · {edit.created_at} · based on revision {edit.base_revision ?? "unknown"}</Text>
        <RichText text={edit.reason} />
        <RichText text={edit.payload.content ?? edit.payload.description ?? edit.payload.engine_name ?? ""} />
        <Text>{edit.piece_names.join(", ")}</Text>
      </Stack>
    </details>)}
    {history.data.proposals.map(proposal => <details key={`proposal-${proposal.id}`}>
      <summary>Card proposal #{proposal.id} · {proposal.deck_name} · {proposal.disposition === "current" ? proposal.status : proposal.disposition}{proposal.superseded_by ? ` → #${proposal.superseded_by}` : ""}</summary>
      <Stack gap={2}>
        <Text tone="faint">{proposal.author} · {proposal.created_at} · based on revision {proposal.base_revision ?? "unknown"}</Text>
        <RichText text={proposal.summary} />
        {proposal.items.map(item => <Stack key={item.id} gap={1}><Text>{item.action} {item.card_name} · {item.status}</Text><RichText text={item.reason} /></Stack>)}
      </Stack>
    </details>)}
  </Stack>;
}
