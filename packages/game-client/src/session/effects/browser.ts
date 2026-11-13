import { GameClient, HttpError, ProtocolError, TransportError, parseSeatBootstrap, readClientConfig, prepareCommand, parsePreparedCommand, PROTOCOL_VERSION } from '@multiverse/game-server-sdk/browser';
import { ReadSession, type SessionEffects } from '../controller.ts';
import { immutable, SessionFailure, type FailureKind } from '../read-state.ts';
import { IndexedDBCommandJournal } from './command-journal.ts';
import { acquireSeat, wait } from './ownership.ts';

function classify(error: unknown): FailureKind {
  if (error instanceof SessionFailure) return error.kind;
  if (error instanceof HttpError) {
    if (error.status === 401 || error.status === 403) return 'unauthorized';
    if (error.status === 429) return 'rate-limited';
    if (error.status === 503) return 'unavailable';
    return 'incompatible';
  }
  if (error instanceof ProtocolError) return 'incompatible';
  if (error instanceof TransportError || (error instanceof Error && error.name === 'AbortError')) return 'disconnected';
  return 'disconnected';
}

/** Deadline covers headers and body decoding, not only the initial fetch. */
async function bounded<T>(signal: AbortSignal, operation: (signal: AbortSignal) => Promise<T>): Promise<T> {
  const deadline = new AbortController();
  const abort = () => deadline.abort();
  if (signal.aborted) deadline.abort();
  signal.addEventListener('abort', abort, { once: true });
  const timer = setTimeout(abort, 30_000);
  try { return await operation(deadline.signal); }
  finally { clearTimeout(timer); signal.removeEventListener('abort', abort); }
}

/** Opt-in factory for future composition. Schema identity must come from the
 * explicitly qualified SDK build; this never enables sends in the current app. */
export function browserCommandSession(protocolSchemaHash: string): ReadSession<File> {
  if (!/^[0-9a-f]{64}$/.test(protocolSchemaHash)) throw new SessionFailure('incompatible');
  return makeBrowserSession(protocolSchemaHash);
}
export function browserReadSession(): ReadSession<File> { return makeBrowserSession(); }
function makeBrowserSession(protocolSchemaHash?: string): ReadSession<File> {
  const effects: SessionEffects<File> = {
    cancellation() { const abort = new AbortController(); return { signal: abort.signal, abort: () => abort.abort() }; },
    acquire: acquireSeat, wait, classify,
    async authorize(file, signal) {
      // Never retain a File, filename, serialized bootstrap or capability in a
      // snapshot. File I/O and SDK construction belong exclusively to this adapter.
      if (file.size > 4096 || file.size < 1) throw new SessionFailure('invalid-file');
      let bootstrap;
      try { bootstrap = parseSeatBootstrap(await file.text()); }
      catch { throw new SessionFailure('invalid-file'); }
      if (signal.aborted) throw new SessionFailure('disconnected');
      const baseURL = window.location.origin;
      const config = await bounded(signal, deadline => readClientConfig(baseURL, { signal: deadline }));
      if (signal.aborted) throw new SessionFailure('disconnected');
      // Both producer/parser contracts require the explicit SocketAddr spelling,
      // including :80. URL.origin alone would lose the default port distinction.
      if (bootstrap.server_origin !== config.server_origin) throw new SessionFailure('invalid-file');
      const { capability, bootstrap_version: _version, ...identity } = bootstrap;
      const binding = immutable(identity);
      const sdk = new GameClient({ baseURL, matchId: binding.match_id, capability });
      const codec = { prepare: prepareCommand, parse: parsePreparedCommand };
      return { binding, ...(protocolSchemaHash ? { commands: {
        protocol: protocolSchemaHash, protocolVersion: PROTOCOL_VERSION, codec,
        journal: new IndexedDBCommandJournal(codec),
        newId: () => Array.from(crypto.getRandomValues(new Uint8Array(16)), byte => byte.toString(16).padStart(2, '0')).join(''),
        transport: {
          submitExact: (body: string, commandSignal: AbortSignal) => bounded(commandSignal, deadline => sdk.submitExact(body, { signal: deadline })),
          async execution(id: string, commandSignal: AbortSignal) {
            try { return await bounded(commandSignal, deadline => sdk.execution(id, { signal: deadline })); }
            catch (error) { if (error instanceof HttpError && error.status === 404) return null; throw error; }
          },
          cancelQueued: (id: string, commandSignal: AbortSignal) => bounded(commandSignal, deadline => sdk.cancel(id, { signal: deadline })),
        },
      } } : {}), transport: {
        manifest: readSignal => bounded(readSignal, deadline => sdk.manifest({ signal: deadline })),
        resume: readSignal => bounded(readSignal, deadline => sdk.resume({ signal: deadline })),
        poll: (after, readSignal) => bounded(readSignal, deadline => sdk.poll({ after, limit: 32, waitMs: 20_000, signal: deadline })),
        async presentation(readSignal) {
          try {
            const { descriptor, artifact } = await bounded(readSignal, deadline => sdk.presentation({ signal: deadline }));
            if (descriptor.catalog_sha256 !== binding.catalog_sha256 || artifact.catalog_sha256 !== binding.catalog_sha256) throw new SessionFailure('incompatible');
            return artifact;
          } catch (error) {
            if (error instanceof HttpError && error.status === 404) return null;
            throw error;
          }
        },
      } };
    },
  };
  return new ReadSession(effects);
}
