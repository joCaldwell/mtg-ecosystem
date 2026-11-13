// Shared by build-time Ajv and its emitted standalone code. These are wire
// formats, not rules quantities (which remain exact decimal strings).
export const numericFormats = Object.fromEntries([
  ['uint8', 255], ['uint32', 4_294_967_295],
  ['uint64', Number.MAX_SAFE_INTEGER], ['uint', Number.MAX_SAFE_INTEGER],
].map(([name, maximum]) => [name, {
  type: 'number', validate: n => Number.isSafeInteger(n) && n >= 0 && n <= maximum,
}]));
