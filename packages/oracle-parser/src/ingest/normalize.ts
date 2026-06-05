// packages/oracle-parser/src/ingest/normalize.ts
// Ingestion text normalization utility

export function normalizeOracleText(text: string): string {
  if (!text) return "";
  
  // 1. Strip reminder text (parentheses and contents)
  // 2. Replace self-reference (e.g. Card Name -> ~)
  // 3. Normalize whitespace
  
  return text
    .replace(/\s*\(.*?\)/g, "") // simple reminder text stripping
    .trim();
}
