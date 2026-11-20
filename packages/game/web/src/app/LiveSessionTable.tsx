import { useMemo, useRef, useState, type ReactNode } from 'react';
import type { ReadSession, ReadSnapshot } from '../session/controller.ts';
import { immutable } from '../session/read-state.ts';
import { liveTable } from '../presentation/live-table.ts';
import { SessionStatus } from '../features/table/SessionStatus.tsx';
import { GameSurface, GameTable as Table } from '@multiverse/game-ui';
import { LiveDecision } from '../features/live/LiveDecision.tsx';
import type { LiveDecisionDraft } from '../session/live-decision.ts';
import { CommandStatus } from '../features/opening/CommandStatus.tsx';
import { LiveActions } from '../features/live/LiveActions.tsx';
import type { LiveActionDraft } from '../session/live-action.ts';
import styles from './LiveSessionTable.module.css';

/** Shared standalone/product composition. Host owns routing; one session owns
 * all authorization, reads, discovery, journal and command effects. */
export function LiveSessionTable({ session, snapshot, headingLevel = 1, emptyActions, diagnostics = false, interactive = true }: {
  readonly session: ReadSession<File>; readonly snapshot: ReadSnapshot;
  readonly headingLevel?: 1 | 2; readonly emptyActions?: ReactNode; readonly diagnostics?: boolean;
  readonly interactive?: boolean;
}) {
  const Heading = headingLevel === 1 ? 'h1' : 'h2';
  const currentContext = session.commandContext();
  const observation = snapshot.read?.state.status === 'available' ? snapshot.read.state.observation : null;
  const request = observation?.decision ?? null;
  const actionIntent = useRef(0);
  const [actionDraft, setActionDraft] = useState<LiveActionDraft | null>(null);
  const [captured, setCaptured] = useState<LiveDecisionDraft | null>(null);
  // Capture on draft creation, including build/catalog/schema and incarnation.
  // Never reconstruct context in a completion callback. Keep the disabled draft
  // across transient reads; identity changes reset DecisionPanel synchronously.
  let draft = captured;
  if (!snapshot.binding && captured) { draft = null; setCaptured(null); }
  else if (currentContext && request && JSON.stringify(currentContext) !== JSON.stringify(captured?.context)) {
    draft = { context: currentContext, request, explanation: observation?.decision_context ?? null }; setCaptured(draft);
  }
  const openingEnabled = interactive && currentContext !== null && JSON.stringify(currentContext) === JSON.stringify(draft?.context);
  const table = useMemo(() => snapshot.read ? immutable(liveTable(snapshot.read, snapshot.metadata)) : null, [snapshot.read, snapshot.metadata]);
  const actionObjects = useMemo(() => {
    if (!table) return [];
    const cards = [...table.hand, ...table.commandZone, ...table.stack, ...(table.otherObjects ?? []),
      ...table.seats.flatMap(seat => [...seat.battlefield, ...seat.commandZone])];
    const permitted = new Set(snapshot.read?.state.status === 'available' ? snapshot.read.state.observation.view.objects.map(object => object.handle) : []);
    return [...new Map(cards.filter(card => permitted.has(card.inspectionId)).map(card => [card.inspectionId, { handle: card.inspectionId, name: card.name }])).values()];
  }, [table, snapshot.read]);
  const decisionLabels = useMemo(() => {
    const cards = table ? [...table.hand, ...table.commandZone, ...table.stack, ...(table.otherObjects ?? []), ...table.seats.flatMap(seat => [...seat.battlefield, ...seat.commandZone])] : [];
    const labels = cards.map(card => ({ handle: card.inspectionId, name: card.name }));
    if (request?.request === 'search_lands') for (const candidate of request.candidates) {
      const metadata = snapshot.metadata?.cards.find(card => card.oracle_id === candidate.oracle_id);
      labels.push({ handle: candidate.handle, name: metadata?.faces.length === 1 ? metadata.faces[0]!.name : null });
    }
    return immutable([...new Map(labels.map(label => [label.handle, label])).values()]);
  }, [table, request, snapshot.metadata]);
  return <GameSurface className={styles.liveSurface}><SessionStatus diagnostics={diagnostics} snapshot={snapshot} onStop={session.stop} />{table ? <Table diagnostics={diagnostics} table={table} headingLevel={headingLevel}
        choiceStatus={<CommandStatus command={snapshot.command} connected={interactive && snapshot.connection === 'ready'} onRetry={() => { if (interactive) void session.retryCommand(); }} />}
        decisionControls={request?.request === 'priority' || request?.request === 'mana' ? <LiveActions snapshot={snapshot.actions} draft={actionDraft} enabled={interactive && currentContext !== null} objects={actionObjects}
          onStart={scope => { const intent = ++actionIntent.current; setActionDraft(null); void session.startActions(scope).then(value => { if (intent === actionIntent.current) setActionDraft(value); }); }} onCancel={generation => { actionIntent.current++; setActionDraft(null); void session.cancelActions(generation); }}
          onSubmit={(original, index) => { void session.submitAction(original, index); }} /> : <LiveDecision draft={draft} request={request} labels={decisionLabels} enabled={openingEnabled} onIntent={intent => { void session.submitIntent(intent); }} />} /> : <section className={styles.page}><Heading>Live table</Heading><p>{snapshot.connection === 'pending' ? 'Waiting for the first committed observation.' : 'Import a single-seat file to open your permitted view.'}</p><CommandStatus command={snapshot.command} connected={interactive && snapshot.connection === 'ready'} onRetry={() => { if (interactive) void session.retryCommand(); }} />{emptyActions}</section>}</GameSurface>;
}
