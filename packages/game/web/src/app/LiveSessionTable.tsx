import { useMemo, useState, type ReactNode } from 'react';
import type { ReadSession, ReadSnapshot } from '../session/controller.ts';
import { immutable } from '../session/read-state.ts';
import { liveTable } from '../presentation/live-table.ts';
import { SessionStatus } from '../features/table/SessionStatus.tsx';
import { GameSurface, GameTable as Table } from '@multiverse/game-ui';
import { LiveDecision } from '../features/live/LiveDecision.tsx';
import type { LiveDecisionDraft } from '../session/live-decision.ts';
import { CommandStatus } from '../features/opening/CommandStatus.tsx';
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
  const decisionLabels = useMemo(() => {
    const cards = table ? [...table.hand, ...table.commandZone, ...table.stack, ...(table.otherObjects ?? []), ...table.seats.flatMap(seat => [...seat.battlefield, ...seat.commandZone])] : [];
    const labels = cards.map(card => ({ handle: card.inspectionId, name: card.name }));
    return immutable([...new Map(labels.map(label => [label.handle, label])).values()]);
  }, [table]);
  return <GameSurface className={styles.liveSurface}><SessionStatus diagnostics={diagnostics} snapshot={snapshot} onStop={session.stop} />{table ? <Table diagnostics={diagnostics} table={table} headingLevel={headingLevel}
        choiceStatus={<CommandStatus command={snapshot.command} connected={interactive && snapshot.connection === 'ready'} onRetry={() => { if (interactive) void session.retryCommand(); }} />}
        decisionControls={<LiveDecision draft={draft} request={request} labels={decisionLabels} enabled={openingEnabled} onIntent={intent => { void session.submitIntent(intent); }} />} /> : <section className={styles.page}><Heading>Live table</Heading><p>{snapshot.connection === 'pending' ? 'Waiting for the first committed observation.' : 'Import a single-seat file to open your permitted view.'}</p><CommandStatus command={snapshot.command} connected={interactive && snapshot.connection === 'ready'} onRetry={() => { if (interactive) void session.retryCommand(); }} />{emptyActions}</section>}</GameSurface>;
}
