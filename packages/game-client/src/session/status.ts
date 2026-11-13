import type { ReadSnapshot } from './controller.ts';

export function connectionLabel(snapshot: ReadSnapshot): string {
  switch (snapshot.connection) {
    case 'idle': return 'Not connected';
    case 'importing': return 'Checking seat file';
    case 'owning': return 'Acquiring seat ownership';
    case 'blocked': return 'Seat open in another tab';
    case 'locks-unavailable': return 'Browser ownership unavailable';
    case 'authenticating': return 'Verifying table identity';
    case 'pending': return 'Waiting for a committed view';
    case 'ready': return `Connected · Seat ${(snapshot.binding?.viewer ?? 0) + 1}`;
    case 'retrying': return `Reconnecting · attempt ${snapshot.retry} of 4`;
    case 'rate-limited': return 'Rate limit or possible contention';
    case 'unavailable': return 'Host temporarily unavailable';
    case 'disconnected': return 'Connection lost';
    case 'unauthorized': return 'Seat authorization rejected';
    case 'incompatible': return 'Incompatible table or presentation';
    case 'invalid-file': return 'Unable to import this seat file';
    case 'ended': return 'Match ended';
  }
}
export function connectionDetail(snapshot: ReadSnapshot): string {
  switch (snapshot.connection) {
    case 'blocked': return 'Close or release the owning tab, then import this seat file again. This tab cannot take over.';
    case 'locks-unavailable': return 'This browser must support Web Locks on the local origin. No host reads have started.';
    case 'invalid-file': return 'Choose the operator’s generated single-seat bootstrap file (up to 4 KiB). The origin must match this client’s configured server.';
    case 'incompatible': return 'The protocol, viewer, match, build, catalog or presentation did not match this session. Import a compatible file after the operator checks the host.';
    case 'unauthorized': return 'The host did not accept this capability. Ask the local operator for a current single-seat file, then reimport.';
    case 'rate-limited': return 'Automatic retries stopped. This may be rate limiting or another client using the seat; it does not establish which client owns it. Resolve the cause before reimporting.';
    case 'unavailable': return 'The host is unavailable and automatic reads have stopped. The retained view may be stale. Reimport when the host is ready.';
    case 'disconnected': return 'Automatic retries stopped. The retained view may be stale. Check the local host and reimport to reconnect.';
    case 'retrying': return `${snapshot.failure === 'rate-limited' ? 'The host reported HTTP 429.' : snapshot.failure === 'unavailable' ? 'The host reported HTTP 503.' : 'The network request failed.'} Retained observations may be stale; no action is sent.`;
    case 'pending': return 'The host has no committed observation for this seat yet. The session will keep reading.';
    case 'ended': return 'The host published a rules outcome. Read polling has stopped and this tab released ownership.';
    case 'ready': return snapshot.command.phase === 'disabled' ? 'Read-only session. No choices or automatic moves are submitted.' : 'Only explicit opening-hand confirmations can send a choice. Later decisions are unavailable.';
    default: return 'The capability stays in memory. Refreshing or closing this page requires importing the file again.';
  }
}
