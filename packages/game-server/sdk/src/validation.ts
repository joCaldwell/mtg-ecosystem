import type { CommandEnvelope } from '../generated/command.js';
import type { StreamPage } from '../generated/stream.js';
import type { SubmissionResult } from '../generated/submission.js';
import type { CommandExecution } from '../generated/execution.js';
import type { PublicManifest } from '../generated/manifest.js';
import { commandValidator, streamValidator, submissionValidator, executionValidator,
  manifestValidator, commandIdValidator, counterValidator } from '../generated/validators.js';
import { PROTOCOL_VERSION } from '../generated/protocol.ts';
export { PROTOCOL_VERSION } from '../generated/protocol.ts';
export class ProtocolError extends Error {
  constructor(message = 'Invalid server protocol data') { super(message); this.name = 'ProtocolError'; }
}
function exactNumbers(value: unknown, depth = 0): void {
  if (depth > 128) throw new ProtocolError('Protocol nesting exceeds limit');
  if (typeof value === 'number' && !Number.isSafeInteger(value)) throw new ProtocolError('Protocol integer exceeds exact JavaScript range');
  if (value && typeof value === 'object') for (const child of Object.values(value)) exactNumbers(child, depth + 1);
}
export function counter(value: string): string {
  if (!counterValidator(value)) throw new ProtocolError('Invalid canonical transport counter');
  return value;
}
export function commandIdentifier(value: string): string {
  if (!commandIdValidator(value)) throw new ProtocolError('Invalid command identifier');
  return value;
}
export function manifest(value: unknown): PublicManifest {
  exactNumbers(value);
  if (!manifestValidator(value) || value.protocol_version !== PROTOCOL_VERSION) throw new ProtocolError('Invalid public manifest');
  return value;
}
export function execution(value: unknown): CommandExecution {
  exactNumbers(value);
  if (!executionValidator(value)) throw new ProtocolError('Invalid command execution status');
  return value;
}
export function command(value: unknown): CommandEnvelope {
  exactNumbers(value);
  if (!commandValidator(value) || value.protocol_version !== PROTOCOL_VERSION) throw new ProtocolError('Invalid command envelope');
  return value;
}
export function submission(value: unknown): SubmissionResult {
  exactNumbers(value);
  if (!submissionValidator(value)) throw new ProtocolError('Invalid submission result');
  return value;
}
export function stream(value: unknown, after: string): StreamPage {
  exactNumbers(value);
  if (!streamValidator(value) || value.protocol_version !== PROTOCOL_VERSION) throw new ProtocolError('Invalid private stream page');
  if (value.entries.length === 0 && value.has_more) throw new ProtocolError('Empty stream page claims more included work');
  let previous = BigInt(counter(after));
  for (const entry of value.entries) {
    const cursor = BigInt(counter(entry.cursor));
    if (cursor <= previous) throw new ProtocolError('Stream cursor did not advance');
    previous = cursor;
  }
  if (BigInt(counter(value.next)) !== previous) throw new ProtocolError('Stream next cursor disagrees with included entries');
  return value;
}
