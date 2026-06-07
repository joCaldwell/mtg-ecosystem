// packages/oracle-parser/src/ingest/normalize.ts
// Ingestion text normalization utility

export function normalizeOracleText(text: string, cardName?: string): string {
  if (!text) return "";
  
  // 1. Strip reminder text (parentheses and contents)
  let normalized = text.replace(/\s*\(.*?\)/g, "");

  // Normalize contractions
  normalized = normalized.replace(/\bit's\b/gi, "it is");
  normalized = normalized.replace(/\byou're\b/gi, "you are");
  normalized = normalized.replace(/\bdon't\b/gi, "dont");
  normalized = normalized.replace(/\bisn't\b/gi, "isnt");

  
  // 2. Replace self-reference (e.g. Card Name -> ~)
  if (cardName) {
    const names = [cardName];
    if (cardName.includes(",")) {
      names.push(cardName.split(",")[0].trim());
    }

    for (const name of names) {
      if (!name) continue;
      const escaped = name.replace(/[-\/\\^$*+?.()|[\]{}]/g, "\\$&");
      
      // Replace possessive forms (e.g., Name's, Name') with "its"
      const possessiveRegex = new RegExp(escaped + "'s?", "gi");
      normalized = normalized.replace(possessiveRegex, "its");

      // Replace base name with "~"
      const nameRegex = new RegExp(escaped, "gi");
      normalized = normalized.replace(nameRegex, "~");
    }
  }

  // 3. Normalize whitespace
  normalized = normalized.replace(/\s+/g, " ").trim();
  
  return normalized;
}
