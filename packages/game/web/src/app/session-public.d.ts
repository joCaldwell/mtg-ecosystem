import type { ReactElement } from 'react';

export interface GameSessionProps {
  readonly gameId: string;
  readonly bootstrapFile: File;
  readonly headingLevel?: 1 | 2;
  /** Disable command controls when the host confirms a pause/result. */
  readonly interactive?: boolean;
  /** Host transport for the app-selected same-origin game proxy. */
  readonly requestFetch?: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
}
/** Real session through the host-selected authenticated game endpoint. Requires
 * the coordinator's human-only bootstrap; authorization stays in memory. */
export declare function GameSession(props: GameSessionProps): ReactElement;
