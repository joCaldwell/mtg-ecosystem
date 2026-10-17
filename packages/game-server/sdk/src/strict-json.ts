import { ProtocolError } from './validation.ts';

const MAX_DEPTH = 128;

/** The caller bounds UTF-8 bytes before calling this raw network decoder. */
export function strictJSON(text: string): unknown {
  let decoded: unknown;
  // Validate the complete JSON grammar first; the scanner below must inspect
  // original lexemes, before duplicate keys and numeric precision disappear.
  try { decoded = JSON.parse(text) as unknown; }
  catch { throw new ProtocolError('Invalid JSON response'); }
  let position = 0;
  const fail = (message: string): never => { throw new ProtocolError(message); };
  const whitespace = (): void => {
    for (;;) {
      const code = text.charCodeAt(position);
      if (code === 0x20 || code === 0x09 || code === 0x0a || code === 0x0d) position++;
      else return;
    }
  };
  const string = (): string => {
    if (text.charCodeAt(position) !== 0x22) return fail('Invalid JSON response');
    const start = position++;
    while (position < text.length) {
      const code = text.charCodeAt(position++);
      if (code === 0x5c) { position++; continue; }
      if (code !== 0x22) continue;
      const value: string = JSON.parse(text.slice(start, position));
      // Rust strings contain Unicode scalar values; JSON.parse alone also
      // accepts escaped lone UTF-16 surrogates, which Rust cannot deserialize.
      for (let i = 0; i < value.length; i++) {
        const unit = value.charCodeAt(i);
        if (unit >= 0xd800 && unit <= 0xdbff) {
          const next = value.charCodeAt(++i);
          if (!(next >= 0xdc00 && next <= 0xdfff)) return fail('Invalid JSON Unicode scalar');
        } else if (unit >= 0xdc00 && unit <= 0xdfff) return fail('Invalid JSON Unicode scalar');
      }
      return value;
    }
    return fail('Invalid JSON response');
  };
  const number = (): void => {
    const start = position;
    while (position < text.length) {
      const code = text.charCodeAt(position);
      if ((code >= 0x30 && code <= 0x39) || code === 0x2d || code === 0x2b || code === 0x2e || code === 0x65 || code === 0x45) position++;
      else break;
    }
    const lexeme = text.slice(start, position);
    if (!/^(?:0|-?[1-9][0-9]*)$/.test(lexeme)) return fail('Noncanonical JSON integer');
    if (!Number.isSafeInteger(Number(lexeme))) return fail('JSON integer exceeds exact JavaScript range');
  };
  const value = (depth: number): void => {
    if (depth > MAX_DEPTH) return fail('Protocol nesting exceeds limit');
    whitespace();
    switch (text.charCodeAt(position)) {
      case 0x7b: { // Object keys are compared after JSON escape decoding.
        position++; whitespace();
        if (text.charCodeAt(position) === 0x7d) { position++; return; }
        const keys = new Set<string>();
        for (;;) {
          whitespace();
          const key = string();
          if (keys.has(key)) return fail('Duplicate JSON object key');
          keys.add(key); whitespace();
          if (text.charCodeAt(position++) !== 0x3a) return fail('Invalid JSON response');
          value(depth + 1); whitespace();
          const separator = text.charCodeAt(position++);
          if (separator === 0x7d) return;
          if (separator !== 0x2c) return fail('Invalid JSON response');
        }
      }
      case 0x5b:
        position++; whitespace();
        if (text.charCodeAt(position) === 0x5d) { position++; return; }
        for (;;) {
          value(depth + 1); whitespace();
          const separator = text.charCodeAt(position++);
          if (separator === 0x5d) return;
          if (separator !== 0x2c) return fail('Invalid JSON response');
        }
      case 0x22: string(); return;
      case 0x74: position += 4; return; // true; grammar was checked above.
      case 0x66: position += 5; return; // false
      case 0x6e: position += 4; return; // null
      default: number(); return;
    }
  };
  value(0); whitespace();
  if (position !== text.length) return fail('Invalid JSON response');
  return decoded;
}
