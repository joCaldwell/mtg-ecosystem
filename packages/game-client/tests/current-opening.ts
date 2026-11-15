import captured from '../fixtures/foundations-opening/opening-seat0-http.json' with { type: 'json' };
import { OBSERVATION_VERSION } from '../src/session/read-state.ts';

// Synthetic current-protocol envelopes reuse permitted historical display data.
// This is a test double, never a recapture or current native qualification. The
// original fixture bytes and provenance remain unchanged and separately tested.
const opening = structuredClone(captured);
opening.resume.state.observation.observation_version = OBSERVATION_VERSION;
export default opening;
