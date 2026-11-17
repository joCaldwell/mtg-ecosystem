import type { OpeningTablePreview as PublicPreview } from '../web/src/preview/public.d.ts';
import type { OpeningTablePreview as Implementation } from '../web/src/preview/OpeningTablePreview.tsx';

// The public declaration intentionally hides qualified protocol build inputs.
// The owning client still proves its concrete component satisfies that API.
type Assert<T extends true> = T;
export type PreviewImplementsPublicContract = Assert<typeof Implementation extends typeof PublicPreview ? true : false>;
export type PublicContractImplementsPreview = Assert<typeof PublicPreview extends typeof Implementation ? true : false>;
