import { useState } from "react";
import { api, type DocumentEdit, type Envelope, type Inbox } from "../../api/client.ts";
import { editDiff, editTitle, isTextEdit } from "../../domain/documents.ts";
import { ago } from "../../domain/time.ts";
import { RichText } from "../../mtg/index.ts";
import { Box, Chip, DiffView, ErrorNote, Eyebrow, Row, Stack, Text } from "../../ui/index.ts";
import { RulingControls } from "./RulingControls.tsx";
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
  const [rejecting, setRejecting] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function rule(outcome: "accept" | "reject", reason = "") {
    setBusy(true);
    setError(null);
    try {
      onRuled(await api.ruleDocumentEdit(edit.id, outcome, reason));
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
          <span className={styles.author}>{edit.author}</span>
          <span>{ago(edit.created_at)}</span>
          {showDeck && <span>{edit.deck_name ?? "Your profile"}</span>}
        </Eyebrow>
        <div className={styles.title}>{editTitle(edit)}</div>
      </Stack>

      <Stack gap={2}>
        <Eyebrow>Why the agent is asking</Eyebrow>
        <RichText text={edit.reason} />
      </Stack>

      {edit.stale && (
        <Box tone="warn" padding="sm" role="status">
          <Text tone="warn">
            This document changed after the edit was proposed. Accepting replaces the current text with the
            proposal. The diff below is against what the agent saw.
          </Text>
        </Box>
      )}

      <Change edit={edit} />

      {!edit.read_only && <RulingControls
        rejecting={rejecting}
        busy={busy}
        acceptLabel="Accept the edit"
        rejectLabel="Reject"
        onAccept={() => rule("accept")}
        onStartReject={() => setRejecting(true)}
        onReject={(reason) => rule("reject", reason)}
        onCancelReject={() => setRejecting(false)}
      />}
      {error && <ErrorNote>{error}</ErrorNote>}

      <Text tone="faint" size="xs">
        This document is yours. Agents can change it only through an edit you accept.
      </Text>
    </Box>
  );
}
