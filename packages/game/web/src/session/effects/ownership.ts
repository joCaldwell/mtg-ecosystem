import type { ReadBinding } from '../read-state.ts';
import { SessionFailure } from '../read-state.ts';

/** The callback promise holds the browser lock for exactly this authorization.
 * ifAvailable never queues a hidden takeover; the user must explicitly reimport. */
export function acquireSeat(binding: ReadBinding, signal: AbortSignal): Promise<null | (() => void)> {
  if (!navigator.locks) return Promise.reject(new SessionFailure('locks-unavailable'));
  if (signal.aborted) return Promise.reject(new SessionFailure('disconnected'));
  const name = `multiverse:read-seat:${binding.server_origin}:${binding.match_id}:${binding.viewer}`;
  return new Promise((resolve, reject) => {
    void navigator.locks.request(name, { mode: 'exclusive', ifAvailable: true }, async lock => {
      if (!lock || signal.aborted) { resolve(null); return; }
      await new Promise<void>(released => {
        let active = true;
        const release = () => {
          if (!active) return;
          active = false;
          signal.removeEventListener('abort', release);
          released();
        };
        signal.addEventListener('abort', release, { once: true });
        resolve(release);
      });
    }).catch(() => reject(new SessionFailure('locks-unavailable')));
  });
}

export function wait(milliseconds: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) { reject(new SessionFailure('disconnected')); return; }
    const cancel = () => { clearTimeout(timer); signal.removeEventListener('abort', cancel); reject(new SessionFailure('disconnected')); };
    const timer = setTimeout(() => { signal.removeEventListener('abort', cancel); resolve(); }, milliseconds);
    signal.addEventListener('abort', cancel, { once: true });
  });
}
