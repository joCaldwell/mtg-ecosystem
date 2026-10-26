import { Explainer } from "../../../ui/index.ts";
import { useVoice } from "../../../app/Preferences.tsx";
// The Brief tab (spec §6): the owner's document about what the deck is for.
// Agents read it every session and can only propose edits to it, which show
// up in place under the section they concern. Archived briefs are read-only.

import { useState } from "react";
import { api, type DocumentEdit, type Engine } from "../../../api/client.ts";
import { useInbox } from "../../../app/Inbox.tsx";
import { Button, ConfirmDialog, Eyebrow, Text } from "../../../ui/index.ts";
import { DocumentEditCard } from "../../decisions/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import { BriefField } from "./BriefField.tsx";
import { EngineCard } from "./EngineCard.tsx";
import { EngineEditor } from "./EngineEditor.tsx";
import styles from "./BriefPanel.module.css";

/** The engine being edited, a new one, or none. */
type EngineForm = { engine?: Engine } | null;

export function BriefPanel() {
  const voice = useVoice();
  const state = useDeckState();
  const { deckId, run, apply } = useDeck();
  const inbox = useInbox();
  const { brief, document_edits: edits } = state;
  const readOnly = state.deck.category === "archived";

  const [form, setForm] = useState<EngineForm>(null);
  const [removing, setRemoving] = useState<Engine | null>(null);

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

  async function save(patch: { thesis?: string; constraints_md?: string }) {
    return (await run(() => api.updateBrief(deckId, patch))) !== null;
  }

  return (
    <>
      <Explainer title="Deck brief" summary={voice("The brief is your plan: what this deck should do and what matters to you.", "Your brief gives every agent the same plan and constraints.")}>Start with a sentence about how you want to win. Add budget or playgroup expectations as constraints. Name an engine when you want the app to check that its pieces are still in the list. Only you approve changes to this plan.</Explainer>
      {edits.filter(e => e.group_id === e.id).map(edit => <DocumentEditCard key={edit.id} edit={edit}
        onRuled={r => { apply(r); inbox.apply(r.inbox); }} />)}
      <BriefField
        readOnly={readOnly}
        title="Thesis"
        value={brief.thesis}
        purpose="What is this deck trying to do, and why would it win?"
        onSave={(thesis) => save({ thesis })}
      >
        {pending(["thesis"])}
      </BriefField>

      <BriefField
        readOnly={readOnly}
        title="Constraints"
        value={brief.constraints_md}
        purpose="Budget, pod expectations, cards you will not play."
        onSave={(constraints_md) => save({ constraints_md })}
      >
        {pending(["constraints"])}
      </BriefField>

      <section className={styles.engines}>
        <div className={styles.head}>
          <Eyebrow>Engines</Eyebrow>
          {!form && !readOnly && (
            <Button size="sm" variant="ghost" onClick={() => setForm({})}>
              Add an engine
            </Button>
          )}
        </div>
        {brief.engines.length === 0 && !form && (
          <Text tone="muted" size="sm" as="p">
            No engines named yet. An engine is a combo or plan the deck is built around; its pieces are checked
            against the list.
          </Text>
        )}
        {brief.engines.map((engine) =>
          form?.engine?.id === engine.id ? (
            <EngineEditor key={engine.id} engine={engine} onClose={() => setForm(null)} />
          ) : (
            <EngineCard
              key={engine.id}
              engine={engine}
              onEdit={() => setForm({ engine })}
              onRemove={() => setRemoving(engine)}
            />
          ),
        )}
        {form && !form.engine && <EngineEditor onClose={() => setForm(null)} />}
        {pending(["engine_set", "engine_remove"])}
      </section>

      <Text tone="faint" size="xs" as="p">
        {readOnly ? "This archived brief is read-only." : voice("Write or edit your plan above. Your list stays as it is.", "The brief is yours. Agents read it every session and can only propose edits to it.")}
      </Text>

      {removing && (
        <ConfirmDialog
          title={`Remove the engine “${removing.name}”?`}
          confirmLabel="Remove the engine"
          tone="danger"
          onClose={() => setRemoving(null)}
          onConfirm={() => {
            const engine = removing;
            setRemoving(null);
            void run(() => api.removeEngine(deckId, engine.id));
          }}
        >
          The engine is taken off the brief. The cards stay in the deck.
        </ConfirmDialog>
      )}
    </>
  );
}
