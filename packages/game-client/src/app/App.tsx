import { Component, useEffect, useMemo, useRef, useState, useSyncExternalStore, type ErrorInfo, type ReactNode } from 'react';
import { Link, NavLink, Route, Routes, useLocation } from 'react-router';
import { browserCommandSession } from '../session/effects/browser.ts';
import { connectionLabel } from '../session/status.ts';
import { immutable } from '../session/read-state.ts';
import { liveTable } from '../presentation/live-table.ts';
import { Join } from '../features/join/Join.tsx';
import { SessionStatus } from '../features/table/SessionStatus.tsx';
import { GameTable as Table } from '@multiverse/game-ui';
import { fixtureTable, missingMetadataTable, decisionGalleryFixture } from '../preview/fixture.ts';
import { DecisionGallery } from '../features/decisions/DecisionGallery.tsx';
import { OpeningTablePreview } from '../preview/OpeningTablePreview.tsx';
import { protocolSchemaHash } from '../../generated/sdk-identity.ts';
import { LiveDecision } from '../features/live/LiveDecision.tsx';
import type { LiveDecisionDraft } from '../session/live-decision.ts';
import { CommandStatus } from '../features/opening/CommandStatus.tsx';
import { LiveActions } from '../features/live/LiveActions.tsx';
import type { LiveActionDraft } from '../session/live-action.ts';
import styles from './App.module.css';

class SurfaceBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  override state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  // Do not serialize errors/component props: future props can contain private data.
  override componentDidCatch(_error: Error, _info: ErrorInfo) {}
  override render() {
    return this.state.failed ? <section className={styles.page} role="alert"><h1>Unable to display this view</h1><p>A saved choice may still be pending. Release the seat or reload, then reimport the same seat file to recover it.</p></section> : this.props.children;
  }
}

export function App() {
  const { hash, pathname } = useLocation();
  const previousRoute = useRef(`${pathname}${hash}`);
  useEffect(() => {
    const titles: Record<string, string> = {
      '/': 'Commander table', '/fixtures/opening': 'Commander table',
      '/fixtures/decisions': hash === '#table' ? 'Opening hand preview' : 'Decision drafts',
      '/fixtures/missing-metadata': 'Missing metadata fixture', '/table': 'Live table',
      '/join': 'Join a Commander table', '/about': 'About this build',
    };
    document.title = `${titles[pathname] ?? 'Page not found'} · Multiverse Commander`;
    const route = `${pathname}${hash}`;
    if (previousRoute.current === route) return;
    previousRoute.current = route;
    // Route changes originate in navigation. Observations/connection updates
    // never run this effect or move focus while someone edits a decision.
    const main = document.getElementById('main');
    const fragment = hash && hash !== '#table' ? document.getElementById(hash.slice(1)) : null;
    const target = fragment ?? main?.querySelector<HTMLElement>('h1') ?? main;
    if (!target) return;
    target.tabIndex = -1;
    if (fragment) fragment.scrollIntoView({ block: 'start' });
    else main?.scrollTo({ top: 0 });
    target.focus({ preventScroll: true });
  }, [pathname, hash]);
  const [session] = useState(() => browserCommandSession(protocolSchemaHash));
  const snapshot = useSyncExternalStore(session.subscribe, session.getSnapshot);
  useEffect(() => () => session.stop(), [session]);
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
  const openingEnabled = currentContext !== null && JSON.stringify(currentContext) === JSON.stringify(draft?.context);
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
  return <>
    <a className="skip" href="#main">Skip to table content</a>
    <header className={styles.header}>
      <Link className={styles.brand} to="/">Multiverse<span>Commander</span></Link>
      <nav aria-label="Primary"><NavLink to="/" end>Table</NavLink><NavLink to="/table">Live table</NavLink><NavLink to="/join">Join</NavLink><NavLink to="/fixtures/decisions">Decision gallery</NavLink><NavLink to="/about">About this build</NavLink></nav>
      <span className={styles.status} role="status">{connectionLabel(snapshot)}</span>
    </header>
    <main id="main" tabIndex={-1}><SurfaceBoundary key={pathname}><Routes>
      <Route path="/" element={<Table table={fixtureTable} />} />
      <Route path="/fixtures/opening" element={<Table table={fixtureTable} />} />
      <Route path="/fixtures/decisions" element={hash === '#table' ? <OpeningTablePreview /> : <DecisionGallery {...decisionGalleryFixture} cards={fixtureTable.hand} />} />
      <Route path="/table" element={<div className={styles.liveSurface}><SessionStatus snapshot={snapshot} onStop={session.stop} />{table ? <Table table={table}
        choiceStatus={<CommandStatus command={snapshot.command} connected={snapshot.connection === 'ready'} onRetry={() => { void session.retryCommand(); }} />}
        decisionControls={request?.request === 'priority' || request?.request === 'mana' ? <LiveActions snapshot={snapshot.actions} draft={actionDraft} enabled={currentContext !== null} objects={actionObjects}
          onStart={scope => { const intent = ++actionIntent.current; setActionDraft(null); void session.startActions(scope).then(value => { if (intent === actionIntent.current) setActionDraft(value); }); }} onCancel={generation => { actionIntent.current++; setActionDraft(null); void session.cancelActions(generation); }}
          onSubmit={(original, index) => { void session.submitAction(original, index); }} /> : <LiveDecision draft={draft} request={request} labels={decisionLabels} enabled={openingEnabled} onIntent={intent => { void session.submitIntent(intent); }} />} /> : <section className={styles.page}><h1>Live table</h1><p>{snapshot.connection === 'pending' ? 'Waiting for the first committed observation.' : 'Import a single-seat file to open your permitted view.'}</p><CommandStatus command={snapshot.command} connected={snapshot.connection === 'ready'} onRetry={() => { void session.retryCommand(); }} /><Link to="/join">Import seat file</Link></section>}</div>} />
      <Route path="/fixtures/missing-metadata" element={<Table table={missingMetadataTable} />} />
      <Route path="/join" element={<Join snapshot={snapshot} onImport={file => { void session.join(file); }} onStop={session.stop} />} />
      <Route path="/about" element={<section className={styles.page}><span className="eyebrow">Local opening-hand client</span><h1>A table with opening choices</h1><p>This build joins a local match through a single-seat file, owns one session and displays committed permitted observations. You can keep, take mulligans and order opening cards. Priority and mana windows offer bounded action discovery. Pass priority is enabled; unexplained card, ability and payment choices remain unavailable.</p><p>Only source-qualified, permitted data belongs on the table. Missing printed text and artwork remain explicitly unavailable.</p><h2>Keyboard use</h2><p>Tab to a card and press Enter to inspect it. Close or Escape returns focus to that card. Jump to decision moves to the decision inspector. Remote updates never move keyboard focus.</p><h2>Session lifetime</h2><p>Your capability stays in memory. Refresh or closing this page requires reimport. Another tab cannot take over an owned seat. Confirmed choices are saved before sending. Refresh preserves uncertain choices for recovery after reimport. Complete M2–M4 play remains separate work.</p><p><Link to="/fixtures/missing-metadata">Inspect the missing-metadata fixture</Link></p><Link to="/fixtures/opening">Return to inspection table</Link></section>} />
      <Route path="*" element={<section className={styles.page}><h1>Page not found</h1><Link to="/">Return to table</Link></section>} />
    </Routes></SurfaceBoundary></main>
  </>;
}
