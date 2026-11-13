import type { CommandEnvelope } from '@multiverse/game-server-sdk';
import { immutable } from './read-state.ts';
import type { JournalBinding, JournalRecord, SeatBinding } from './contracts.ts';

export const JOURNAL_VERSION = 1;
export const MAX_COMMAND_BYTES = 65_536;
export const MAX_JOURNAL_RECORDS = 64;
export type JournalFailureKind = 'unavailable' | 'blocked' | 'full' | 'corrupt' | 'conflict' | 'failed';
export class JournalFailure extends Error {
  readonly kind: JournalFailureKind;
  constructor(kind: JournalFailureKind) { super(`command-journal-${kind}`); this.kind = kind; }
}
export interface PreparedCommand { readonly envelope: CommandEnvelope; readonly exactBody: string }
export interface CommandCodec {
  prepare(envelope: CommandEnvelope): PreparedCommand;
  parse(exactBody: string): PreparedCommand;
}
export function seatKey(binding: SeatBinding): string { return JSON.stringify([binding.origin, binding.match, binding.seat]); }
export function sameBinding(a: JournalBinding, b: JournalBinding): boolean {
  return bindingKeys.every(key => a[key] === b[key]);
}
function object(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
}
const bindingKeys = ['origin', 'match', 'seat', 'engineBuild', 'serverBuild', 'catalog', 'protocol'] as const;
/** Strict local record decoding. Wire decoding is exclusively injected SDK code.
 * Stored records contain only a binding and exact command, never a capability,
 * observation, diagnostic, or redundant unvalidated envelope. */
export function decodeRecord(value: unknown, codec: CommandCodec): JournalRecord<CommandEnvelope> {
  try {
    if (!object(value, ['version', 'binding', 'exactBody']) || value.version !== JOURNAL_VERSION
      || !object(value.binding, bindingKeys)
      || !bindingKeys.every(key => {
        const item = (value.binding as Record<string, unknown>)[key];
        return typeof item === 'string' && item.length > 0 && item.length <= 512;
      })
      || typeof value.exactBody !== 'string' || value.exactBody.length > MAX_COMMAND_BYTES) throw new Error();
    const binding = value.binding as unknown as JournalBinding;
    const origin = new URL(binding.origin);
    if (!/^https?:\/\/[^/?#@]+$/.test(binding.origin) || origin.username !== '' || origin.password !== '' || !['http:', 'https:'].includes(origin.protocol)
      || !/^[0-9a-f]{1,64}$/.test(binding.match) || !/^[0-3]$/.test(binding.seat)
      || ![binding.engineBuild, binding.serverBuild, binding.catalog, binding.protocol].every(item => /^[0-9a-f]{64}$/.test(item))) throw new Error();
    const prepared = codec.parse(value.exactBody);
    return immutable({ version: 1, binding: { ...binding }, ...prepared });
  } catch { throw new JournalFailure('corrupt'); }
}
export function storedRecord(record: JournalRecord<CommandEnvelope>): unknown {
  return { version: record.version, binding: record.binding, exactBody: record.exactBody };
}
