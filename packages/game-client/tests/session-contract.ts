import type { GameSession as PublicGameSession, GameSessionProps as PublicProps } from '../src/app/session-public.d.ts';
import type { GameSession, GameSessionProps } from '../src/app/GameSession.tsx';

// Consumer declarations remain independent of generated SDK inputs, while this
// package proves parity with the implementation against the qualified SDK.
const implementation: typeof PublicGameSession = null! as typeof GameSession;
const publicProps: PublicProps = null! as GameSessionProps;
const implementationProps: GameSessionProps = null! as PublicProps;
void implementation; void publicProps; void implementationProps;
