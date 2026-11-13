import { GameClient, type Observation, type PlayerRequest, type CommandEnvelope,
  type ClientOptions, type PollOptions, type SeatResume, type SeatBootstrap } from '@multiverse/game-server-sdk';
import { GameClient as BrowserClient, type Observation as BrowserObservation } from '@multiverse/game-server-sdk/browser';

const options: ClientOptions = { baseURL: 'http://127.0.0.1:3000', matchId: 'abc', capability: 'fixture', fetch: globalThis.fetch };
const poll: PollOptions = { after: '0', waitMs: 0 };
const browser: GameClient = new BrowserClient(options);
const page = browser.poll(poll);
declare const observation: Observation;
const identical: BrowserObservation = observation;
const decision: PlayerRequest | null = identical.decision;
// @ts-expect-error generated transport counters are decimal strings
const command: CommandEnvelope = { protocol_version: 1, command_id: 'a'.repeat(32), expected_revision: 0, command: { kind: 'concede' } };
void [page, decision, command];
declare const resumed: SeatResume;
declare const bootstrap: SeatBootstrap;
const resumedView: Observation | null = resumed.state.status === 'available' ? resumed.state.observation : null;
const seat: number = bootstrap.viewer;
void [resumedView, seat];
