// The Brief tab (spec §6): the owner's document about what the deck is for.
// Agents read it every session and can only propose edits to it, which show
// up in place under the section they concern. Archived briefs are read-only.

import { useLayoutEffect, useRef, useState } from "react";
import { api, type DocumentEdit, type Engine } from "../../../api/client.ts";
import { useInbox } from "../../../app/Inbox.tsx";
import { Button, ConfirmDialog, ErrorNote, Eyebrow, Text } from "../../../ui/index.ts";
import { DocumentEditCard } from "../../decisions/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import { useAttempt } from "../dialogs/useAttempt.ts";
import { BriefField } from "./BriefField.tsx";
import { EngineCard } from "./EngineCard.tsx";
import { EngineEditor } from "./EngineEditor.tsx";
import styles from "./BriefPanel.module.css";

/** The engine being edited, a new one, or none. */
type EngineForm = { engine?: Engine } | null;

export function BriefPanel() {
  const state = useDeckState();
  const { deckId, apply } = useDeck();
  const inbox = useInbox();
  const { brief, document_edits: edits } = state;
  const readOnly = state.deck.category === "archived";

  const [form, setForm] = useState<EngineForm>(null);
  const [removing, setRemoving] = useState<Engine | null>(null);
  const removal = useAttempt();
  const engines = useRef<HTMLElement>(null);
  const add = useRef<HTMLButtonElement>(null);
  const editsRef = useRef(new Map<number, HTMLButtonElement>());
  const restoreFocus = useRef<EngineForm>(null);

  useLayoutEffect(() => {
    if (form || !restoreFocus.current) return;
    const engine = restoreFocus.current.engine;
    restoreFocus.current = null;
    (engine ? editsRef.current.get(engine.id) : add.current)?.focus({ preventScroll: true });
  }, [form]);

  function closeForm() {
    const active = document.activeElement;
    if (active === document.body || engines.current?.contains(active)) restoreFocus.current = form;
    setForm(null);
  }

  const pending = (kinds: DocumentEdit["kind"][]) =>
    edits
      .filter((e) => e.group_id == null && kinds.includes(e.kind))
      .map((e) => (
        <DocumentEditCard
          key={e.id}
          edit={e}
          onRuled={(r) => {
            apply(r);
            inbox.apply(r.inbox);
          }}
        />
      ));

  return (
    <>
      {edits.filter(e => e.group_id === e.id).map(edit => <DocumentEditCard key={edit.id} edit={edit}
        onRuled={r => { apply(r); inbox.apply(r.inbox); }} />)}
      <BriefField
        readOnly={readOnly}
        title="Thesis"
        value={brief.thesis}
        purpose="What is this deck trying to do, and why would it win?"
        onSave={(thesis) => api.updateBrief(deckId, { thesis })}
      >
        {pending(["thesis"])}
      </BriefField>

      <BriefField
        readOnly={readOnly}
        title="Constraints"
        value={brief.constraints_md}
        purpose="Budget, pod expectations, cards you will not play."
        onSave={(constraints_md) => api.updateBrief(deckId, { constraints_md })}
      >
        {pending(["constraints"])}
      </BriefField>

      <section className={styles.engines} ref={engines}>
        <div className={styles.head}>
          <Eyebrow as="h2">Engines</Eyebrow>
          {!form && !readOnly && (
            <Button ref={add} size="sm" reach onClick={() => setForm({})}>
              Add an engine
            </Button>
          )}
        </div>
        {brief.engines.length === 0 && !form && (
          <Text tone="muted" size="sm" as="p">
            No engines yet
          </Text>
        )}
        {brief.engines.map((engine) =>
          form?.engine?.id === engine.id ? (
            <EngineEditor key={engine.id} engine={engine} onClose={closeForm} />
          ) : (
            <EngineCard
              key={engine.id}
              engine={engine}
              editRef={node => {
                if (node) editsRef.current.set(engine.id, node);
                else editsRef.current.delete(engine.id);
              }}
              onEdit={() => setForm({ engine })}
              onRemove={() => { removal.setError(null); setRemoving(engine); }}
            />
          ),
        )}
        {form && !form.engine && <EngineEditor onClose={closeForm} />}
        {pending(["engine_set", "engine_remove"])}
      </section>

      {readOnly && <Text tone="faint" size="xs" as="p">This archived brief is read-only.</Text>}

      {removing && (
        <ConfirmDialog
          title={`Remove the engine “${removing.name}”?`}
          confirmLabel={removal.busy ? "Removing…" : "Remove the engine"}
          tone="danger"
          busy={removal.busy}
          onClose={() => setRemoving(null)}
          onConfirm={async () => { if (await removal.mutate(() => api.removeEngine(deckId, removing.id))) setRemoving(null); }}
        >
          The engine is taken off the brief. The cards stay in the deck.
          {removal.error && <ErrorNote>{removal.error}</ErrorNote>}
        </ConfirmDialog>
      )}
    </>
  );
}
