import type { SeatResume } from '@multiverse/game-server-sdk';
import type { PresentationArtifact } from '../../generated/presentation.d.ts';
import opening from '../../fixtures/foundations-opening/opening-seat0-http.json';
import presentation from '../../fixtures/foundations-opening/presentation.json';
import { openingFixtureTable } from '../presentation/opening-fixture.ts';

// Exact fixture hashes and both producer schemas are checked before build. These
// assertions are only for immutable checked-in fixtures, never network/file input.
export const fixtureTable = openingFixtureTable(opening.resume as SeatResume, presentation as PresentationArtifact);
// Deliberately omit metadata to exercise the documented text/identity fallback.
// This changes no observation or card fact and is labeled separately in the UI.
export const missingMetadataTable = {
  ...openingFixtureTable(opening.resume as SeatResume, { ...presentation, cards: [] } as PresentationArtifact),
  title: 'Missing metadata fixture',
};
