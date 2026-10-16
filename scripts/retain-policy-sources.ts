// Official policy HTML is retained separately from the native source closure.
// No network refresh: the reviewed native fixture supplies exact URLs/hashes.
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";

const FIXTURE = "packages/game-engine/tests/fixtures/commander-policy.json";
const sha = (bytes: string | Buffer) => createHash("sha256").update(bytes).digest("hex");
export interface NativeSourceFile { path: string; contents: string; sha256: string }
export interface RetainedPolicySource {
  kind: "commander_ban" | "offensive_cards";
  url: string;
  html_sha256: string;
  retained_path: string;
}
interface PolicySource {
  kind: RetainedPolicySource["kind"];
  url: string;
  html_sha256: string;
  environment: string;
  default_file: string;
}
function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Invalid native Commander policy fixture.");
  return value as Record<string, unknown>;
}
function description(value: unknown, source: Omit<PolicySource, "url" | "html_sha256">): PolicySource {
  const row = object(value);
  if (typeof row.url !== "string" || !row.url.startsWith("https://")
    || typeof row.html_sha256 !== "string" || !/^[0-9a-f]{64}$/u.test(row.html_sha256)) {
    throw new Error(`Invalid ${source.kind} URL/hash in native Commander policy fixture.`);
  }
  return { ...source, url: row.url, html_sha256: row.html_sha256 };
}
function policySources(files: readonly NativeSourceFile[]): PolicySource[] {
  const matches = files.filter(file => file.path === FIXTURE);
  if (matches.length === 0) return [];
  if (matches.length !== 1 || sha(matches[0].contents) !== matches[0].sha256) {
    throw new Error("Invalid native Commander policy fixture path/hash.");
  }
  const fixture = object(JSON.parse(matches[0].contents));
  return [
    description(fixture, {
      kind: "commander_ban", environment: "MTG_ENGINE_COMMANDER_BAN_POLICY_PATH",
      default_file: "generated/policy-sources/banned-restricted-20261015.html",
    }),
    description(fixture.offensive_policy, {
      kind: "offensive_cards", environment: "MTG_ENGINE_OFFENSIVE_POLICY_PATH",
      default_file: "generated/policy-sources/offensive-policy-20200610.html",
    }),
  ];
}
function target(store: string, source: PolicySource): string {
  return path.resolve(store, "policy-sources", `${source.kind}-${source.html_sha256}.html`);
}
export function retainPolicySources(
  files: readonly NativeSourceFile[], projectRoot: string, store: string,
  environment: NodeJS.ProcessEnv = process.env,
): RetainedPolicySource[] {
  // Validate every input before creating any retained policy file.
  const checked = policySources(files).map(source => {
    const input = path.resolve(projectRoot, environment[source.environment] ?? source.default_file);
    let bytes: Buffer;
    try { bytes = fs.readFileSync(input); }
    catch (cause) { throw new Error(`Missing pinned ${source.kind} policy source: ${input}`, { cause }); }
    if (sha(bytes) !== source.html_sha256) throw new Error(`Pinned ${source.kind} policy source digest mismatch: ${input}`);
    return { source, bytes };
  });
  return checked.map(({ source, bytes }) => {
    const retained_path = target(store, source);
    fs.mkdirSync(path.dirname(retained_path), { recursive: true });
    fs.writeFileSync(retained_path, bytes);
    return { kind: source.kind, url: source.url, html_sha256: source.html_sha256, retained_path };
  });
}
export function verifyRetainedPolicySources(
  files: readonly NativeSourceFile[], retained: readonly RetainedPolicySource[], store: string,
): void {
  const expected = policySources(files);
  if (retained.length !== expected.length) throw new Error("Retained policy source set differs from native fixture.");
  for (const source of expected) {
    const rows = retained.filter(row => row.kind === source.kind);
    if (rows.length !== 1 || rows[0].url !== source.url || rows[0].html_sha256 !== source.html_sha256
      || rows[0].retained_path !== target(store, source)) {
      throw new Error(`Retained ${source.kind} policy metadata differs from native fixture/store.`);
    }
    let bytes: Buffer;
    try { bytes = fs.readFileSync(rows[0].retained_path); }
    catch (cause) { throw new Error(`Missing retained ${source.kind} policy source.`, { cause }); }
    if (sha(bytes) !== source.html_sha256) throw new Error(`Retained ${source.kind} policy source digest mismatch.`);
  }
}
