import { useState, type ReactNode } from 'react';
import { DecisionPanel } from '../features/decisions/DecisionPanel.tsx';
import { GameTable as Table } from '@multiverse/game-ui';
import { decisionGalleryFixture, fixtureTable } from './fixture.ts';

/** Local review of an unchanged producer-captured request. No session, storage,
 * transport or simulated game progression: confirming only updates this message. */
export function OpeningTablePreview({ headingLevel = 1, title = 'Opening hand preview', toolbar }: { headingLevel?: 1 | 2; title?: string; toolbar?: ReactNode } = {}) {
  const [receipt, setReceipt] = useState('');
  return <Table headingLevel={headingLevel} toolbar={toolbar} table={{ ...fixtureTable, title }} decisionControls={<>
    <p>Choices stay in this preview.</p>
    <DecisionPanel {...decisionGalleryFixture} embedded onComplete={({ response }) => {
      if (response.response === 'mulligan') {
        setReceipt(response.take ? 'Preview confirmed: take a mulligan.' : 'Preview confirmed: keep this hand.');
      }
    }} />
    <p role="status" aria-live="polite">{receipt}</p>
  </>} />;
}
