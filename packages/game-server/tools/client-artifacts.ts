/** Offline allowlist producer. Reads compiler-pinned raw source bytes; never
 * interprets executable IR, fetches cards, or computes runtime rules. */
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
export const sha256 = (value: string | Buffer): string => createHash('sha256').update(value).digest('hex');
export function canonical(value: any): string {
  if (value === null || typeof value !== 'object') return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${canonical(value[key])}`).join(',')}}`;
}
const hashSchema = { type: 'string', pattern: '^[0-9a-f]{64}$' };
const textSchema = { type: ['string', 'null'], maxLength: 16384 };
function object(properties: Record<string, unknown>) {
  return { type: 'object', additionalProperties: false, required: Object.keys(properties), properties };
}
export const presentationSchema = {
  $schema: 'http://json-schema.org/draft-07/schema#',
  ...object({
    presentation_version: { const: 1 }, catalog_sha256: hashSchema,
    cards: { type: 'array', maxItems: 1000, items: object({
      oracle_id: { type: 'string' }, printing_id: { type: 'string' }, source_sha256: hashSchema,
      // No mapping is inferred from Oracle lines or compiler indices in v1.
      ability_mapping: { const: 'unavailable' },
      faces: { type: 'array', minItems: 1, maxItems: 8, items: object({
        face_index: { type: 'integer', minimum: 0, maximum: 7 },
        name: { type: 'string', minLength: 1, maxLength: 1024 },
        oracle_text: textSchema, mana_cost: textSchema, type_line: textSchema,
        image_url: { type: ['string', 'null'], maxLength: 2048 },
      }) },
    }) },
  }),
};
export function presentation(catalog: any) {
  if (!Array.isArray(catalog.definitions) || catalog.definitions.length > 1000) throw new Error('catalog definition bound');
  const seen = new Set<string>();
  const cards = catalog.definitions.map((definition: any) => {
    const p = definition.provenance;
    if (typeof p?.source_json !== 'string' || sha256(p.source_json) !== p.source_sha256) throw new Error('source digest mismatch');
    const source = JSON.parse(p.source_json);
    if (source.oracle_id !== definition.oracle_id || source.id !== p.printing_id || seen.has(source.oracle_id)) throw new Error('source identity mismatch');
    seen.add(source.oracle_id);
    const sourceFaces = source.card_faces ?? [source];
    if (!Array.isArray(sourceFaces) || sourceFaces.length < 1 || sourceFaces.length > 8) throw new Error('source face bound');
    const text = (value: unknown) => {
      if (value === undefined) return null;
      if (typeof value !== 'string' || value.length > 16384) throw new Error('source text bound');
      return value;
    };
    return {
      oracle_id: source.oracle_id, printing_id: source.id, source_sha256: p.source_sha256,
      ability_mapping: 'unavailable',
      faces: sourceFaces.map((face: any, face_index: number) => {
        if (typeof face.name !== 'string' || !face.name.length || face.name.length > 1024) throw new Error('source name bound');
        // Multi-face entries only use artwork explicitly stored on that face.
        const image_url = face.image_uris?.normal ?? null;
        if (image_url !== null) {
          if (typeof image_url !== 'string' || image_url.length > 2048) throw new Error('source image bound');
          const url = new URL(image_url);
          if (url.protocol !== 'https:' || url.username || url.password) throw new Error('source image scheme');
        }
        return { face_index, name: face.name, oracle_text: text(face.oracle_text), mana_cost: text(face.mana_cost), type_line: text(face.type_line), image_url };
      }),
    };
  });
  return { presentation_version: 1, catalog_sha256: sha256(canonical(catalog)), cards };
}
export function inventory(schema: any) {
  function variants(section: any, name: string, tag: string) {
    const definition = section.definitions?.[name];
    const rows = definition?.oneOf ?? definition?.anyOf;
    if (!Array.isArray(rows)) throw new Error(`missing generated ${name}`);
    return rows.map((row: any) => {
      const property = row.properties?.[tag];
      const variant = property?.const ?? property?.enum?.[0];
      if (typeof variant !== 'string') throw new Error(`missing generated ${tag}`);
      return { variant, fields: Object.keys(row.properties).sort(), required: row.required ?? [] };
    });
  }
  return { inventory_version: 1, requests: variants(schema.stream, 'PlayerRequest', 'request'), responses: variants(schema.command, 'PlayerResponse', 'response') };
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [catalogPath, schemaPath, output] = process.argv.slice(2);
  if (!catalogPath || !schemaPath || !output) throw new Error('usage: client-artifacts.ts CATALOG PROTOCOL_SCHEMA NEW_OUTPUT_DIRECTORY');
  const catalogBytes = readFileSync(catalogPath);
  const schemaBytes = readFileSync(schemaPath);
  const artifact = presentation(JSON.parse(catalogBytes.toString()));
  const requests = inventory(JSON.parse(schemaBytes.toString()));
  mkdirSync(output); // create-new directory, no overwriting retained evidence
  const outputs = { 'presentation.json': artifact, 'presentation.schema.json': presentationSchema, 'request-inventory.json': requests };
  const files = Object.entries(outputs).map(([name, value]) => {
    const body = canonical(value) + '\n';
    writeFileSync(resolve(output, name), body, { flag: 'wx' });
    return { path: name, sha256: sha256(body) };
  });
  writeFileSync(resolve(output, 'artifacts.json'), canonical({
    artifact_version: 1, producer_sha256: sha256(readFileSync(fileURLToPath(import.meta.url))),
    catalog_file_sha256: sha256(catalogBytes), catalog_sha256: artifact.catalog_sha256,
    protocol_schema_sha256: sha256(schemaBytes), files,
  }) + '\n', { flag: 'wx' });
}
