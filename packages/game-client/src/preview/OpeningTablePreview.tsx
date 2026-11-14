import { useState } from 'react';
import { DecisionPanel } from '../features/decisions/DecisionPanel.tsx';
import { GameTable as Table } from '@multiverse/game-ui';
import { decisionGalleryFixture, fixtureTable } from './fixture.ts';

/** Local review of an unchanged producer-captured request. No session, storage,
 * transport or simulated game progression: confirming only updates this message. */
export function OpeningTablePreview() {
  const [receipt, setReceipt] = useState('No preview choice confirmed.');
  return <Table table={{ ...fixtureTable, title: 'Opening hand preview' }} decisionControls={<>
    <p>Interactive preview. Choices stay in this page; the captured game does not advance.</p>
    <DecisionPanel {...decisionGalleryFixture} embedded onComplete={({ response }) => {
      if (response.response === 'mulligan') {
        setReceipt(response.take ? 'Preview confirmed: take a mulligan. No game action sent.' : 'Preview confirmed: keep this hand. No game action sent.');
      }
    }} />
    <p role="status" aria-live="polite">{receipt}</p>
  </>} />;
}
