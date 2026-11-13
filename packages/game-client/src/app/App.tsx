import { Component, useEffect, useMemo, useState, useSyncExternalStore, type ErrorInfo, type ReactNode } from 'react';
import { Link, NavLink, Route, Routes } from 'react-router';
import { browserCommandSession } from '../session/effects/browser.ts';
import { connectionLabel } from '../session/status.ts';
import { immutable } from '../session/read-state.ts';
import { liveTable } from '../presentation/live-table.ts';
import { Join } from '../features/join/Join.tsx';
import { SessionStatus } from '../features/table/SessionStatus.tsx';
import { Table } from '../features/table/Table.tsx';
import { fixtureTable, missingMetadataTable, decisionGalleryFixture } from './fixture.ts';
import { DecisionGallery } from '../features/decisions/DecisionGallery.tsx';
import { protocolSchemaHash } from '../../generated/sdk-identity.ts';
import { LiveOpening, type OpeningDraft } from '../features/opening/LiveOpening.tsx';
import { CommandStatus } from '../features/opening/CommandStatus.tsx';
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
  const [session] = useState(() => browserCommandSession(protocolSchemaHash));
  const snapshot = useSyncExternalStore(session.subscribe, session.getSnapshot);
  useEffect(() => () => session.stop(), [session]);
  const currentContext = session.commandContext();
  const request = snapshot.read?.state.status === 'available' ? snapshot.read.state.observation.decision ?? null : null;
  const [captured, setCaptured] = useState<OpeningDraft | null>(null);
  // Capture on draft creation, including build/catalog/schema and incarnation.
  // Never reconstruct context in a completion callback. Keep the disabled draft
  // across transient reads; identity changes reset DecisionPanel synchronously.
  let draft = captured;
  if (!snapshot.binding && captured) { draft = null; setCaptured(null); }
  else if (currentContext && request && JSON.stringify(currentContext) !== JSON.stringify(captured?.context)) {
    draft = { context: currentContext, request }; setCaptured(draft);
  }
  const openingEnabled = currentContext !== null && JSON.stringify(currentContext) === JSON.stringify(draft?.context);
  const table = useMemo(() => snapshot.read ? immutable(liveTable(snapshot.read, snapshot.metadata)) : null, [snapshot.read, snapshot.metadata]);
  const openingLabels = useMemo(() => immutable(table?.hand.map(card => ({ handle: card.inspectionId, name: card.name })) ?? []), [table]);
  return <>
    <a className="skip" href="#main">Skip to table content</a>
    <header className={styles.header}>
      <Link className={styles.brand} to="/">Multiverse<span>Commander</span></Link>
      <nav aria-label="Main navigation"><NavLink to="/" end>Table</NavLink><NavLink to="/table">Live table</NavLink><NavLink to="/join">Join</NavLink><NavLink to="/about">About this build</NavLink></nav>
      <span className={styles.status} role="status">{connectionLabel(snapshot)}</span>
    </header>
    <main id="main" tabIndex={-1}><SurfaceBoundary><Routes>
      <Route path="/" element={<Table table={fixtureTable} />} />
      <Route path="/fixtures/opening" element={<Table table={fixtureTable} />} />
      <Route path="/fixtures/decisions" element={<DecisionGallery {...decisionGalleryFixture} />} />
      <Route path="/table" element={<><SessionStatus snapshot={snapshot} onStop={session.stop} /><CommandStatus command={snapshot.command} connected={snapshot.connection === 'ready'} onRetry={() => { void session.retryCommand(); }} />{table ? <Table table={table} decisionControls={<LiveOpening draft={draft} request={request} labels={openingLabels} enabled={openingEnabled} onIntent={intent => { void session.submitIntent(intent); }} />} /> : <section className={styles.page}><h1>Live table</h1><p>{snapshot.connection === 'pending' ? 'Waiting for the first committed observation.' : 'Import a single-seat file to open your permitted view.'}</p><Link to="/join">Import seat file</Link></section>}</>} />
      <Route path="/fixtures/missing-metadata" element={<Table table={missingMetadataTable} />} />
      <Route path="/join" element={<Join snapshot={snapshot} onImport={file => { void session.join(file); }} onStop={session.stop} />} />
      <Route path="/about" element={<section className={styles.page}><span className="eyebrow">Local opening-hand client</span><h1>A table with opening choices</h1><p>This build joins a local match through a single-seat file, owns one session and displays committed permitted observations. You can keep, take mulligans and order opening cards. Later decisions remain unavailable.</p><p>Only source-qualified, permitted data belongs on the table. Missing printed text and artwork remain explicitly unavailable.</p><h2>Keyboard use</h2><p>Tab to a card and press Enter to inspect it. Close or Escape returns focus to that card. Jump to decision moves to the decision inspector. Remote updates never move keyboard focus.</p><h2>Session lifetime</h2><p>Your capability stays in memory. Refresh or closing this page requires reimport. Another tab cannot take over an owned seat. Confirmed opening choices are saved before sending. Refresh preserves uncertain choices for recovery after reimport. Complete M2–M4 play remains separate work.</p><p><Link to="/fixtures/missing-metadata">Inspect the missing-metadata fixture</Link></p><Link to="/fixtures/opening">Return to inspection table</Link></section>} />
      <Route path="*" element={<section className={styles.page}><h1>Page not found</h1><Link to="/">Return to table</Link></section>} />
    </Routes></SurfaceBoundary></main>
  </>;
}
