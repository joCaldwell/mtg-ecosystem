import { useEffect, useState } from "react";
import { api, type DocumentEdit, type Envelope, type Inbox } from "../../api/client.ts";
import { editDiff, editTitle, isTextEdit } from "../../domain/documents.ts";
import { ago } from "../../domain/time.ts";
import { RichText } from "../../mtg/index.ts";
import { Box, Checkbox, Chip, DiffView, ErrorNote, Eyebrow, Row, Stack, Text } from "../../ui/index.ts";
import { DecisionControls } from "./DecisionControls.tsx";
import styles from "./DocumentEditCard.module.css";

function Change({ edit }: { edit: DocumentEdit }) {
  if (isTextEdit(edit)) {
    // DiffView gives an empty line no height; a non-breaking space holds it open.
    const lines = editDiff(edit).map((l) => (l.text === "" ? { ...l, text: "\u00a0" } : l));
    return lines.length ? (
      <DiffView lines={lines} />
    ) : (
      <Text tone="muted">This edit changes nothing: the text is the same.</Text>
    );
  }
  const name = edit.payload.engine_name ?? "unnamed";
  if (edit.kind === "engine_remove") {
    return (
      <Text tone="soft">
        Accepting removes the engine “{name}” from this deck’s brief. The cards stay in the deck.
      </Text>
    );
  }
  return (
    <Stack gap={4}>
      <Text weight="medium">{name}</Text>
      {edit.payload.description && <RichText text={edit.payload.description} />}
      <Row gap={3} wrap>
        {edit.piece_names.length ? (
          edit.piece_names.map((piece) => <Chip key={piece}>{piece}</Chip>)
        ) : (
          <Text tone="faint">No pieces listed.</Text>
        )}
      </Row>
    </Stack>
  );
}

/**
 * A proposed edit to a document the owner writes (spec §6): the brief or the
 * profile. Ruled on where it is shown, because the change itself is the whole
 * decision; there is no list of cards to read it against.
 */
export function DocumentEditCard({
  edit,
  onRuled,
  showDeck = false,
}: {
  edit: DocumentEdit;
  onRuled: (result: Envelope & { inbox: Inbox }) => void;
  showDeck?: boolean;
}) {
  const group = edit.group_edits.length ? edit.group_edits : [edit];
  const stale = group.some(e => e.stale);
  const reviewKey = JSON.stringify(group.map(e => [e.id, e.current_revision, e.current]));
  const [acknowledged, setAcknowledged] = useState(false);
  const [rejecting, setRejecting] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => setAcknowledged(false), [reviewKey]);

  async function rule(outcome: "accept" | "reject", reason = "") {
    setBusy(true);
    setError(null);
    try {
      onRuled(await api.ruleDocumentEdit(edit.id, outcome, reason, { expected_revision: edit.current_revision ?? undefined, acknowledge_stale: acknowledged, expected_documents: Object.fromEntries(group.map(e => [e.id, e.current])) }));
      setRejecting(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Box tone="agent" className={styles.card}>
      <Stack gap={2}>
        <Eyebrow className={styles.eyebrow}>
          <span className={styles.kind}>{edit.kind === "profile" ? "Profile edit" : edit.kind === "power_target" ? "Power target edit" : "Brief edit"}</span>
          <span className={styles.author_display ?? styles.author}>{edit.author_display ?? edit.author}</span>
          <span>{ago(edit.created_at)}</span>
          {showDeck && <span>{edit.deck_name ?? "Your profile"}</span>}
        </Eyebrow>
        <div className={styles.title}>{editTitle(edit)}</div>
      </Stack>

      <Stack gap={2}>
        <Eyebrow>Why the agent is asking</Eyebrow>
        <RichText text={edit.reason} />
      </Stack>

      {stale && (
        <Box tone="warn" padding="sm" role="status">
          <Text tone="warn">
            The deck or document changed after this proposal. Review the current text and all related changes before accepting.
            The diff below is against the proposal’s original base.
          </Text>
          <Checkbox checked={acknowledged} onChange={setAcknowledged} label="I reviewed the changed deck/document and approve replacing it." />
        </Box>
      )}

      {edit.supersedes.length > 0 && <Text tone="faint">Replaces edits {edit.supersedes.join(", ")}; earlier drafts remain in proposal history.</Text>}
      {group.map(change => <Stack key={change.id} gap={2}>
        {group.length > 1 && <Text weight="medium">{editTitle(change)}</Text>}
        {change.id !== edit.id && <RichText text={change.reason} />}
        <Change edit={change} />
        {change.stale && <Stack gap={2}>
          <Eyebrow>Current document</Eyebrow>
          {change.kind.startsWith("engine_") ? change.current_engine ? <>
            <Text weight="medium">{change.current_engine.name}</Text>
            <RichText text={change.current_engine.description} />
            <Row gap={3} wrap>{change.current_engine.pieces.map(piece => <Chip key={piece.oracle_id}>{piece.name}</Chip>)}</Row>
          </> : <Text tone="faint">This engine is absent.</Text> : <RichText text={change.current || "(empty)"} />}
        </Stack>}
      </Stack>)}

      {!edit.read_only && <DecisionControls
        rejecting={rejecting}
        busy={busy}
        acceptLabel={group.length > 1 ? "Accept all related edits" : "Accept the edit"}
        rejectLabel="Reject"
        onAccept={() => { if (stale && !acknowledged) { setError("Review the current document and acknowledge the changes first."); return; } void rule("accept"); }}
        onStartReject={() => setRejecting(true)}
        onReject={(reason) => rule("reject", reason)}
        onCancelReject={() => setRejecting(false)}
      />}
      {error && <ErrorNote>{error}</ErrorNote>}
    </Box>
  );
}
