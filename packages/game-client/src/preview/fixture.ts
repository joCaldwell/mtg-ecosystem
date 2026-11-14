import type { SeatResume } from '@multiverse/game-server-sdk';
import type { PresentationArtifact } from '../../generated/presentation.d.ts';
import opening from '../../fixtures/foundations-opening/opening-seat0-http.json';
import presentation from '../../fixtures/foundations-opening/presentation.json';
import { openingFixtureTable } from '../presentation/opening-fixture.ts';

// Exact fixture hashes and both producer schemas are checked before build. These
// assertions are only for immutable checked-in fixtures, never network/file input.
export const fixtureTable = openingFixtureTable(opening.resume as SeatResume, presentation as PresentationArtifact);
const resume = opening.resume as SeatResume;
if (resume.state.status !== 'available' || !resume.state.observation.decision) throw new Error('Expected captured opening decision');
export const decisionGalleryFixture = {
  request: resume.state.observation.decision,
  binding: { origin: 'fixture:foundations-opening', match: resume.match_id, seat: resume.viewer, incarnation: 0, revision: resume.next, token: resume.state.observation.decision.token },
  labels: fixtureTable.hand.map(card => ({ handle: card.inspectionId, name: card.name })),
  source: `${fixtureTable.sourceLabel} · capture source 5ff4fd94b6dace1cdee0f0223305e054c42af8f3 · SHA-256 cb0214f247ccb0c651f6cb97534539420e0e1a3eb18867dd75509742335d87d9`,
};
// Deliberately omit metadata to exercise the documented text/identity fallback.
// This changes no observation or card fact and is labeled separately in the UI.
export const missingMetadataTable = {
  ...openingFixtureTable(opening.resume as SeatResume, { ...presentation, cards: [] } as PresentationArtifact),
  title: 'Missing metadata fixture',
};
