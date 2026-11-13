import type { TableView } from '../presentation/table-model.ts';

/** An empty UI shell, not a game fixture. Replaced by a source-qualified producer
 * artifact only; no card facts, game status or decisions are invented here. */
export const pendingTable: TableView = {
  title: 'Commander table',
  sourceLabel: 'Awaiting the native lane’s source-qualified permitted observation.',
  seats: [0, 1, 2, 3].map(seat => ({ label: `Seat ${seat + 1}`, isViewer: false, life: null, handCount: null, libraryCount: null, battlefield: [] })),
  hand: [], commandZone: [], stack: [], turn: null, phase: null, priority: null, decision: null,
};
