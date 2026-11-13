import { Component, type ErrorInfo, type ReactNode } from 'react';
import { Link, NavLink, Route, Routes } from 'react-router';
import { Table } from '../features/table/Table.tsx';
import { fixtureTable, missingMetadataTable } from './fixture.ts';
import styles from './App.module.css';

class SurfaceBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  override state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  // Do not serialize errors/component props: future props can contain private data.
  override componentDidCatch(_error: Error, _info: ErrorInfo) {}
  override render() {
    return this.state.failed ? <section className={styles.page} role="alert"><h1>Unable to display this view</h1><p>No game action was sent. Reload to reopen the inspection fixture.</p></section> : this.props.children;
  }
}

export function App() {
  return <>
    <a className="skip" href="#main">Skip to table content</a>
    <header className={styles.header}>
      <Link className={styles.brand} to="/">Multiverse<span>Commander</span></Link>
      <nav aria-label="Main navigation"><NavLink to="/" end>Table</NavLink><NavLink to="/join">Join</NavLink><NavLink to="/about">About this build</NavLink></nav>
      <span className={styles.status} role="status">Not connected</span>
    </header>
    <main id="main" tabIndex={-1}><SurfaceBoundary><Routes>
      <Route path="/" element={<Table table={fixtureTable} />} />
      <Route path="/fixtures/missing-metadata" element={<Table table={missingMetadataTable} />} />
      <Route path="/join" element={<section className={styles.page}><span className="eyebrow">Local session</span><h1>Join a Commander table</h1><p>Live connections are not available in this build.</p><p>The connected client will import a single-seat credential file from your local operator. Refreshing will require reimporting that file.</p><p>No credentials are collected or stored by this inspection build.</p><Link to="/">Return to inspection table</Link></section>} />
      <Route path="/about" element={<section className={styles.page}><span className="eyebrow">Foundation checkpoint</span><h1>An inspection surface</h1><p>This build provides a desktop table, card inspector and decision inspector. It cannot join a match or submit choices.</p><p>Only source-qualified, permitted data belongs on the table. Missing printed text and artwork remain explicitly unavailable.</p><h2>Keyboard use</h2><p>Tab to a card and press Enter to inspect it. Close or Escape returns focus to that card. Jump to decision moves to the decision inspector.</p><h2>Integration still required</h2><p>SDK integration, seat authorization, ownership locks, coherent resume, exact command journaling and all live gameplay acceptance remain separate gates.</p><p><Link to="/fixtures/missing-metadata">Inspect the missing-metadata fixture</Link></p><Link to="/">Return to table</Link></section>} />
      <Route path="*" element={<section className={styles.page}><h1>Page not found</h1><Link to="/">Return to table</Link></section>} />
    </Routes></SurfaceBoundary></main>
  </>;
}
