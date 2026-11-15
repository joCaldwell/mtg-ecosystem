import { useEffect, useState, useSyncExternalStore } from 'react';
import { GameSurface } from '@multiverse/game-ui';
import { browserCommandSession } from '../session/effects/browser.ts';
import { protocolSchemaHash } from '../../generated/sdk-identity.ts';
import { LiveSessionTable } from './LiveSessionTable.tsx';
import styles from './GameSession.module.css';

export interface GameSessionProps {
  readonly gameId: string;
  /** Obtained by the host's authenticated coordinator. Memory only. */
  readonly bootstrapFile: File;
  readonly headingLevel?: 1 | 2;
  /** Host-owned pause/result disables input without settling saved commands. */
  readonly interactive?: boolean;
}
function OwnedGameSession({ gameId, bootstrapFile, headingLevel = 2, interactive = true }: GameSessionProps) {
  const [session] = useState(() => browserCommandSession(protocolSchemaHash, { endpoint: `/api/games/${gameId}/native` }));
  const snapshot = useSyncExternalStore(session.subscribe, session.getSnapshot);
  useEffect(() => { void session.join(bootstrapFile); return () => session.stop(); }, [session, bootstrapFile]);
  return <GameSurface className={styles.surface}><LiveSessionTable session={session} snapshot={snapshot} headingLevel={headingLevel} interactive={interactive} /></GameSurface>;
}
/** Router-free real session, separate from the captured preview. An imported
 * bootstrap never selects a proxy target; the host explicitly selects gameId. */
export function GameSession(props: GameSessionProps) {
  return /^[a-f0-9]{1,64}$/.test(props.gameId) ? <OwnedGameSession key={props.gameId} {...props} /> : <p role="alert">Game session is unavailable.</p>;
}
