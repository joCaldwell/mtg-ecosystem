import type { ReactElement } from 'react';

export interface GameSessionProps {
  readonly gameId: string;
  readonly bootstrapFile: File;
  readonly headingLevel?: 1 | 2;
}
/** Real session through the host-selected authenticated game endpoint. Requires
 * the coordinator's human-only bootstrap; authorization stays in memory. */
export declare function GameSession(props: GameSessionProps): ReactElement;
