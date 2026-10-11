import { createHash } from "node:crypto";
import { readPaperEvidence } from "../../../scripts/lib/paper-evidence.ts";
import type { ScryfallCard } from "../../../scripts/lib/scryfall.ts";

export function paperEvidenceFixture(printings: ScryfallCard[] = []) {
  const source = Buffer.from(JSON.stringify(printings));
  return readPaperEvidence(Buffer.from(JSON.stringify({ schemaVersion: 1,
    source: { sha256: createHash("sha256").update(source).digest("hex"), bytes: source.length,
      records: printings.length, format: "json", downloadUri: "https://example.invalid/printings.json", updatedAt: "2026-10-10T00:00:00Z" },
    printings })));
}
