import type { GameSession as PublicGameSession, GameSessionProps as PublicProps } from '../web/src/app/session-public.d.ts';
import type { GameSession, GameSessionProps } from '../web/src/app/GameSession.tsx';
import type { isQualifiedProductSession as PublicQualification, ProductSessionIdentity as PublicIdentity } from '../web/src/session/qualification-public.d.ts';
import type { isQualifiedProductSession, ProductSessionIdentity } from '../web/src/session/qualification.ts';

// Consumer declarations remain independent of generated SDK inputs, while this
// package proves parity with the implementation against the qualified SDK.
const implementation: typeof PublicGameSession = null! as typeof GameSession;
const publicProps: PublicProps = null! as GameSessionProps;
const implementationProps: GameSessionProps = null! as PublicProps;
void implementation; void publicProps; void implementationProps;
const qualification: typeof PublicQualification = null! as typeof isQualifiedProductSession;
const publicIdentity: PublicIdentity = null! as ProductSessionIdentity;
const implementationIdentity: ProductSessionIdentity = null! as PublicIdentity;
void qualification; void publicIdentity; void implementationIdentity;
