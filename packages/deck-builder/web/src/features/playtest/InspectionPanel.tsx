import { useEffect, useState } from "react";
import { api } from "../../api/client.ts";
import type {
  DeckState,
  Memory,
  PlaytestAction,
  Zone,
} from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import {
  ZONE_LABELS,
  faceOf,
  objectZone,
  observeObject,
} from "../../domain/table.ts";
import { CardInspection, RichText } from "../../mtg/index.ts";
import {
  Button,
  CounterControl,
  Dot,
  MultiSelect,
  Select,
  TextArea,
  TextField,
} from "../../ui/index.ts";
import type { PlaytestController } from "./usePlaytest.ts";
import styles from "./InspectionPanel.module.css";
export function InspectionPanel({
  controller: c,
  deck,
  objectId,
  pinned,
  onDeckReload,
}: {
  controller: PlaytestController;
  deck: DeckState;
  objectId: string | null;
  pinned: boolean;
  onDeckReload: () => void;
}) {
  const s = c.view!.session,
    f = s.frame,
    object = objectId ? f.objects[objectId] : null,
    card = object ? s.cards[object.oracle_id] : null;
  const [tagDeck, setTagDeck] = useState(deck);
  useEffect(() => setTagDeck(deck), [deck]);
  const entry =
    card && tagDeck.cards.find((x) => x.oracle_id === card.oracle_id);
  const history = useQuery<{ memories: Memory[] }>(
    () =>
      card
        ? api.cardHistory(s.deck_id, card.oracle_id)
        : api.listMemories({ scope: "deck", deck: s.deck_id }),
    [s.deck_id, card?.oracle_id],
  );
  const [counterName, setCounterName] = useState("+1/+1"),
    [tagName, setTagName] = useState(""),
    [tagError, setTagError] = useState<string | null>(null);
  const [tagBusy, setTagBusy] = useState(false),
    [memoryStatus, setMemoryStatus] = useState<string | null>(null),
    [savingMemory, setSavingMemory] = useState(false);
  const [manualPower, setManualPower] = useState(""),
    [manualToughness, setManualToughness] = useState("");
  const target = pinned && card ? card.oracle_id : null;
  useEffect(() => {
    setManualPower(object?.power ?? "");
    setManualToughness(object?.toughness ?? "");
  }, [object?.id, object?.power, object?.toughness]);
  useEffect(() => {
    if (!c.ui.draft.text && c.ui.draft.oracle_id !== target)
      c.editUi({
        draft: {
          ...c.ui.draft,
          oracle_id: target,
          request_id: crypto.randomUUID(),
        },
      });
  }, [target, c.ui.draft.text]);
  const busy = c.pending > 0,
    playing = f.opening.stage === "playing";
  const act = (action: PlaytestAction) => {
    if (!busy && playing) void c.act(action);
  };
  const memories = history.data?.memories ?? [];
  async function tag(ids: string[]) {
    if (!card || !entry || tagBusy) return;
    const oracleId = card.oracle_id;
    setTagBusy(true);
    setTagError(null);
    try {
      const result = await api.updateCard(s.deck_id, oracleId, {
        tag_ids: ids.map(Number),
      });
      setTagDeck(result.state);
      onDeckReload();
    } catch (e) {
      setTagError(e instanceof Error ? e.message : String(e));
    } finally {
      setTagBusy(false);
    }
  }
  async function createTag() {
    setTagBusy(true);
    setTagError(null);
    try {
      const result = await api.createTag(s.deck_id, tagName);
      setTagDeck(result.state);
      setTagName("");
      onDeckReload();
    } catch (e) {
      setTagError(e instanceof Error ? e.message : String(e));
    } finally {
      setTagBusy(false);
    }
  }
  async function saveMemory() {
    setSavingMemory(true);
    setMemoryStatus(null);
    const success = await c.saveObservation();
    setMemoryStatus(
      success
        ? "Observation saved to the deck."
        : "Save failed. Your text remains here; retry when ready.",
    );
    setSavingMemory(false);
    if (success) {
      history.reload();
      onDeckReload();
    }
  }
  const draftCard = c.ui.draft.oracle_id ? s.cards[c.ui.draft.oracle_id] : null;
  return (
    <div className={styles.panel}>
      <div className={styles.body}>
        {card && object ? (
          <CardInspection
            observation={observeObject(c.view!, object.id)}
            context={[
              ZONE_LABELS[objectZone(c.view!, object.id)!],
              object.role === "card" ? null : object.role,
              object.token ? "token copy" : null,
            ].filter(Boolean).join(" · ")}
          >
            {card.faces.length > 1 && (
              <label>
                Card face
                <Select
                  aria-label="Card face"
                  disabled={busy}
                  value={object.face}
                  onChange={(e) => {
                    if (!busy)
                      void c.act({
                        type: "edit",
                        id: object.id,
                        face: Number(e.target.value),
                      });
                  }}
                >
                  {card.faces.map((face, i) => (
                    <option key={i} value={i}>
                      {face.name}
                    </option>
                  ))}
                </Select>
              </label>
            )}
            {!pinned && (
              <Button
                onClick={() =>
                  c.editUi({ pinned: object.id, selected: [object.id] })
                }
              >
                Pin this card
              </Button>
            )}
          </CardInspection>
        ) : (
          <section>
            <h2>{s.deck_name}</h2>
            <details className={styles.checks}>
              <summary>
                Deck checks · {deck.checks.length}{" "}
                {deck.checks.length === 1 ? "finding" : "findings"}
              </summary>
              <ul className={styles.findings}>
                {deck.checks.map((finding) => (
                  <li key={finding.key}>
                    <Dot tone={finding.severity === "error" ? "cut" : "warn"} />
                    <span>
                      <strong>{finding.title}</strong> {finding.detail}
                    </span>
                  </li>
                ))}
                {!deck.checks.length && (
                  <li>
                    <Dot tone="add" />
                    <span>Deck checks are clear.</span>
                  </li>
                )}
              </ul>
            </details>
          </section>
        )}
        {pinned && object && card && (
          <details className={styles.section}>
            <summary>Edit card state</summary>
            <div className={styles.state}>
            <div className={styles.row}>
              <TextField
                aria-label="Counter name"
                value={counterName}
                onChange={(e) => setCounterName(e.target.value)}
              />
              <Button
                size="sm"
                disabled={busy || !playing || !counterName.trim()}
                onClick={() =>
                  act({
                    type: "counter",
                    id: object.id,
                    name: counterName.trim(),
                    delta: 1,
                  })
                }
              >
                Add counter
              </Button>
            </div>
            {Object.entries(object.counters).map(([name, value]) => (
              <div key={name} className={styles.row}>
                <span>{name}</span>
                <CounterControl
                  label={`${card.name} ${name} counters`}
                  value={value}
                  disabled={busy || !playing}
                  onChange={(next) =>
                    act({
                      type: "counter",
                      id: object.id,
                      name,
                      delta: next - value,
                    })
                  }
                />
              </div>
            ))}
            <p className={styles.muted}>
              Printed {faceOf(card, object.face).power ?? "—"}/
              {faceOf(card, object.face).toughness ?? "—"}
            </p>
            <div className={styles.row}>
              <TextField
                aria-label="Manual power"
                value={manualPower}
                onChange={(e) => setManualPower(e.target.value)}
                placeholder="Power"
              />
              <TextField
                aria-label="Manual toughness"
                value={manualToughness}
                onChange={(e) => setManualToughness(e.target.value)}
                placeholder="Toughness"
              />
              <Button
                size="sm"
                disabled={busy || !playing}
                onClick={() =>
                  act({
                    type: "edit",
                    id: object.id,
                    power: manualPower.trim() || null,
                    toughness: manualToughness.trim() || null,
                  })
                }
              >
                Apply
              </Button>
            </div>
            {object.role === "commander" && (
              <div className={styles.row}>
                <span>Commander casts</span>
                <CounterControl
                  label={`${card.name} commander casts`}
                  value={object.cast_count}
                  disabled={busy || !playing}
                  onChange={(next) =>
                    next >= 0 &&
                    act({ type: "edit", id: object.id, cast_count: next })
                  }
                />
              </div>
            )}
            {f.zones.battlefield.includes(object.id) && (
              <label>
                Attached to
                <Select
                  aria-label="Attachment host"
                  value={object.attached_to ?? ""}
                  disabled={busy || !playing}
                  onChange={(e) =>
                    act({
                      type: "attach",
                      id: object.id,
                      host: e.target.value || null,
                    })
                  }
                >
                  <option value="">Not attached</option>
                  {f.zones.battlefield
                    .filter((id) => id !== object.id)
                    .map((id) => (
                      <option key={id} value={id}>
                        {s.cards[f.objects[id].oracle_id].name} ·{" "}
                        {id.slice(0, 4)}
                      </option>
                    ))}
                </Select>
              </label>
            )}
            </div>
          </details>
        )}
        {pinned && card && (
          <section className={styles.section}>
            <h3>Deck-card tags</h3>
            {entry && !object?.token ? (
              <>
                <MultiSelect
                  label="Assign deck-card tags"
                  placeholder={tagBusy ? "Saving tags…" : "Select tags"}
                  options={tagDeck.tags.map((t) => ({
                    value: String(t.id),
                    label: t.name,
                  }))}
                  value={entry.tag_ids.map(String)}
                  onChange={tag}
                  footer={
                    <div className={styles.row}>
                      <TextField
                        aria-label="New tag name"
                        value={tagName}
                        onChange={(e) => setTagName(e.target.value)}
                        placeholder="New tag name"
                      />
                      <Button
                        size="sm"
                        disabled={tagBusy || !tagName.trim()}
                        onClick={() => void createTag()}
                      >
                        Create tag
                      </Button>
                    </div>
                  }
                />
              </>
            ) : (
              <p className={styles.muted}>
                {object?.token
                  ? "Tokens have no deck-card entry. Save a card-linked observation instead."
                  : "This card is no longer in the saved deck. Card-linked observations are still available."}
              </p>
            )}
            {tagError && (
              <p role="alert" className={styles.error}>
                {tagError}
              </p>
            )}
          </section>
        )}
        <section className={styles.section}>
          <h3>Observation</h3>
          <p className={styles.muted}>
            About {draftCard ? draftCard.name : "the whole deck"} · tested r
            {c.ui.draft.deck_revision ?? s.deck_revision}
          </p>
          {c.ui.draft.text && c.ui.draft.oracle_id !== target && (
            <Button
              size="sm"
              onClick={() =>
                c.editUi({
                  draft: {
                    ...c.ui.draft,
                    oracle_id: target,
                    request_id: crypto.randomUUID(),
                  },
                })
              }
            >
              {target
                ? "Link this draft to the selected card"
                : "Make this a deck observation"}
            </Button>
          )}
          <TextArea
            aria-label="Playtest observation"
            rows={5}
            value={c.ui.draft.text}
            placeholder="What did you learn about this hand or early turns?"
            onChange={(e) => {
              setMemoryStatus(null);
              c.editUi({ draft: { ...c.ui.draft, text: e.target.value } });
            }}
          />
          <Button
            variant="primary"
            disabled={!c.ui.draft.text.trim() || savingMemory}
            onClick={() => void saveMemory()}
          >
            {savingMemory ? "Saving observation…" : "Save observation"}
          </Button>
          {memoryStatus && <p role="status">{memoryStatus}</p>}
        </section>
        <section className={styles.section}>
          <h3>Existing memories</h3>
          {history.error && <p role="alert">{history.error}</p>}
          {memories.map((m) => (
            <div key={m.id} className={styles.memory}>
              <RichText text={m.text} />
              <small>
                {m.author_display ?? m.author} ·{" "}
                {m.deck_revision === null ? "global" : `r${m.deck_revision}`} ·
                #{m.id}
              </small>
            </div>
          ))}
          {!memories.length && (
            <p className={styles.muted}>No observations linked here yet.</p>
          )}
        </section>
      </div>
    </div>
  );
}
