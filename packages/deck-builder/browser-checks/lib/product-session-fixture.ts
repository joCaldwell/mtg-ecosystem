// Dev-only seeding may use backend internals. The product still uses ./host.
import { createHash, createHmac, randomBytes, randomUUID } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, lstatSync, readdirSync, mkdirSync } from "node:fs";
import { join, resolve, sep } from "node:path";
import { createServer } from "node:net";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import { isDeepStrictEqual } from "node:util";
import type { GameIdentity, GameRecord } from "../../src/contracts.ts";

export const OBSERVATION_VERSION = 21;
const frozen16 = "565a1204d6e4b14443b9f49dda2ecbc6b46ee85a";
export const repository = fileURLToPath(new URL("../../../../", import.meta.url));
export const sha256 = (bytes: string | Uint8Array) => createHash("sha256").update(bytes).digest("hex");
export const fileHash = (path: string) => sha256(readFileSync(path));
export function check(value: unknown, name: string): asserts value {
  // Never interpolate actual values, SDK errors, browser DOM or credentials.
  if (!value) throw new Error(name);
}
export function completeResponse(actual: unknown, expected: unknown) {
  check(isDeepStrictEqual(actual, expected), "complete_response");
}
export interface Inputs {
  nativeRoot: string; sdkRoot: string; qualification: string; records: string; recordsHash: string;
}
interface Pins {
  native_source: string; sdk_source: string; engine_build_id: string; server_build_id: string;
  protocol_schema_sha256: string; bootstrap_schema_sha256: string;
  native_source_manifest_sha256: string;
  native_source_verification_sha256?: string;
  sdk_build_manifest_sha256?: string; sdk_verification_sha256?: string;
  sdk_evidence_root?: string; sdk_source_bridge_sha256?: string;
}
function confined(root: string, path: string) {
  const result = resolve(root, path);
  check(result.startsWith(resolve(root) + sep), "artifact_path");
  check(lstatSync(result).isFile() && !lstatSync(result).isSymbolicLink(), "artifact_file");
  return result;
}
function sdkFile(root: string, path: string) {
  // The separate public Node producer's build manifest deliberately includes
  // these two original tools. No other parent traversal is an accepted input.
  return path === "../tools/client-artifacts.ts" || path === "../tools/presentation.schema.json"
    ? confined(resolve(root, ".."), path.slice(3)) : confined(root, path);
}
/** A caller must supply the parent's NEW qualification/pins. Historical 565/16
 * cannot be relabelled by a CLI switch. No native process runs before this gate. */
export function verifyInputs(inputs: Inputs): Pins {
  const pins: Pins = JSON.parse(readFileSync(inputs.qualification, "utf8"));
  check(/^[a-f0-9]{40}$/.test(pins.native_source) && pins.native_source !== frozen16
    && /^[a-f0-9]{40}$/.test(pins.sdk_source) && pins.sdk_source !== frozen16, "new_qualified_source_required");
  const native = (name: string) => confined(inputs.nativeRoot, name);
  check(fileHash(native("protocol.schema.json")) === pins.protocol_schema_sha256, "protocol_pin");
  check(fileHash(native("bootstrap.schema.json")) === pins.bootstrap_schema_sha256, "bootstrap_pin");
  check(fileHash(native("source-files.json")) === pins.native_source_manifest_sha256, "native_source_manifest_pin");
  const schema = JSON.parse(readFileSync(native("protocol.schema.json"), "utf8"));
  check(schema.resume.definitions.Observation.properties.observation_version
    && pins.protocol_schema_sha256 !== "2eb874b2fa9e306b57d7da05d2e9a26b6965f2a639711d6705981e790d906ba5", "new_observation_schema_required");
  const info = JSON.parse(readFileSync(native("native-info.json"), "utf8"));
  check(info.engine_build_id === pins.engine_build_id && info.server_build_id === pins.server_build_id, "native_build_pin");
  const copies = JSON.parse(readFileSync(native("native-copies-safe.json"), "utf8"));
  const binaryHashes = copies.binaries ?? copies.binary_hashes;
  check(copies.source_unchanged && binaryHashes && fileHash(native("mtg-server")) === binaryHashes["mtg-server"], "native_artifact_digest");
  const compiled = JSON.parse(readFileSync(native("source-files.json"), "utf8")) as { path: string; sha256: string }[];
  check(Array.isArray(compiled) && compiled.length > 0 && compiled.length <= 2_000
    && new Set(compiled.map(row => row.path)).size === compiled.length, "compiled_source_manifest");
  // Always check the retained COMPILED closure. Moving main's unused Rust
  // source is neither rebuilt nor relabelled by this browser-only fixture.
  const frozenSource = resolve(inputs.nativeRoot, "../source");
  for (const row of compiled) {
    check(typeof row.path === "string" && /^[a-f0-9]{64}$/.test(row.sha256), "compiled_source_row");
    check(fileHash(confined(frozenSource, row.path)) === row.sha256, "frozen_native_source_changed");
  }
  const sdk = resolve(repository, "packages/game-server/sdk");
  const build = JSON.parse(readFileSync(confined(inputs.sdkRoot, "dist/build.json"), "utf8"));
  check(build.schema_sha256 === pins.protocol_schema_sha256
    && fileHash(confined(inputs.sdkRoot, "generated/generation.json")) === build.generation_sha256, "sdk_generation_pin");
  const generation = JSON.parse(readFileSync(confined(inputs.sdkRoot, "generated/generation.json"), "utf8"));
  check(generation.bootstrap_schema_sha256 === pins.bootstrap_schema_sha256, "sdk_bootstrap_pin");
  const verification = JSON.parse(readFileSync(confined(inputs.sdkRoot, "generated/verification.json"), "utf8"));
  let sdkBuildPin = pins.sdk_build_manifest_sha256, sdkVerificationPin = pins.sdk_verification_sha256;
  if (pins.sdk_source !== pins.native_source) {
    check(pins.sdk_evidence_root && pins.sdk_source_bridge_sha256, "followup_sdk_bridge_required");
    const bridgePath = confined(pins.sdk_evidence_root, "sdk-source-bridge.json");
    check(fileHash(bridgePath) === pins.sdk_source_bridge_sha256, "followup_sdk_bridge_pin");
    const bridge = JSON.parse(readFileSync(bridgePath, "utf8"));
    check(bridge.native_source === pins.native_source && bridge.head === pins.sdk_source
      && bridge.native_binary_sha256 === binaryHashes["mtg-server"], "followup_native_binding");
    const changedPaths = ["packages/game-server/sdk/README.md", "packages/game-server/sdk/package.json", "packages/game-server/sdk/scripts/build-browser.mjs"];
    check(bridge.changed_original_rows.length === 3 && changedPaths.every(path => bridge.changed_original_rows.filter((row: { path: string }) => row.path === path).length === 1), "only_documented_sdk_changes");
    check(compiled.length === 898 && bridge.unchanged_original_rows === 895 && verification.source_files.length === 50, "source_closure_counts");
    for (const row of compiled) {
      if (!row.path.startsWith("packages/game-server/sdk/")) continue;
      const changed = bridge.changed_original_rows.find((entry: { path: string }) => entry.path === row.path);
      if (changed) check(changed.original_sha256 === row.sha256, "documented_sdk_original");
      check(fileHash(sdkFile(inputs.sdkRoot, row.path.slice("packages/game-server/sdk/".length))) === (changed?.current_sha256 ?? row.sha256), "undocumented_sdk_diff");
    }
    sdkBuildPin = bridge.sdk_build_sha256; sdkVerificationPin = bridge.sdk_verification_sha256;
    check(fileHash(confined(inputs.sdkRoot, "dist/build.json")) === sdkBuildPin
      && fileHash(confined(inputs.sdkRoot, "generated/verification.json")) === sdkVerificationPin, "followup_sdk_receipt_pin");
  } else {
    check(pins.native_source_verification_sha256 && pins.sdk_evidence_root && pins.sdk_source_bridge_sha256, "coqualified_source_proof_required");
    check(fileHash(native("source-verification.json")) === pins.native_source_verification_sha256, "coqualified_native_verification_pin");
    const sourceProof = JSON.parse(readFileSync(native("source-verification.json"), "utf8"));
    check(sourceProof.source_sha === pins.native_source && sourceProof.source_rows === compiled.length
      && sourceProof.source_manifest_sha256 === pins.native_source_manifest_sha256
      && sourceProof.engine_build_id === pins.engine_build_id && sourceProof.server_build_id === pins.server_build_id
      && sourceProof.source_before_after_equal && sourceProof.all_manifest_rows_match_git_head
      && sourceProof.engine_and_server_independently_rederived, "coqualified_native_source_binding");
    for (const name of ["mtg-server", "provision_client"]) {
      check(sourceProof.binary_hashes[name] === binaryHashes[name] && fileHash(native(name)) === binaryHashes[name], "coqualified_native_binary_pin");
    }
    const bridgePath = confined(pins.sdk_evidence_root, "sdk-source-bridge.json");
    check(fileHash(bridgePath) === pins.sdk_source_bridge_sha256, "coqualified_sdk_bridge_pin");
    const bridge = JSON.parse(readFileSync(bridgePath, "utf8"));
    check(bridge.bridge_version === 2 && bridge.native_source === pins.native_source && bridge.head === pins.sdk_source
      && bridge.native_source_manifest_sha256 === pins.native_source_manifest_sha256
      && bridge.native_source_verification_sha256 === pins.native_source_verification_sha256
      && bridge.native_binary_sha256 === binaryHashes["mtg-server"] && bridge.compiled_source_rows === compiled.length
      && bridge.sdk_source_files === verification.source_files.length && bridge.all_sdk50_inputs_byte_equal, "coqualified_sdk_source_binding");
    for (const row of compiled) {
      if (row.path.startsWith("packages/game-server/sdk/")) check(fileHash(sdkFile(inputs.sdkRoot, row.path.slice("packages/game-server/sdk/".length))) === row.sha256, "coqualified_sdk_source_changed");
    }
    check(sdkBuildPin === bridge.sdk_build_sha256 && sdkVerificationPin === bridge.sdk_verification_sha256
      && fileHash(confined(inputs.sdkRoot, "dist/build.json")) === sdkBuildPin
      && fileHash(confined(inputs.sdkRoot, "generated/verification.json")) === sdkVerificationPin, "coqualified_sdk_receipt_pin");
  }
  check(verification.status === "preliminary-sdk-checks-passed" && verification.generation_manifest_sha256 === build.generation_sha256
    && verification.source_files.some((row: { path: string }) => row.path === "src/policies.ts"), "sdk_qualification_receipt");
  for (const row of [...build.sources, ...build.files.map((row: { path: string; sha256: string }) => ({ ...row, path: `dist/${row.path}` }))]) {
    check(fileHash(sdkFile(inputs.sdkRoot, row.path)) === row.sha256, "qualified_sdk_digest");
    check(fileHash(sdkFile(sdk, row.path)) === row.sha256, "installed_sdk_differs");
  }
  for (const row of verification.source_files) {
    check(fileHash(confined(inputs.sdkRoot, row.path)) === row.sha256 && fileHash(confined(sdk, row.path)) === row.sha256, "qualified_sdk_source");
  }
  check(fileHash(confined(sdk, "dist/build.json")) === fileHash(confined(inputs.sdkRoot, "dist/build.json")), "installed_sdk_build");
  check(fileHash(inputs.records) === inputs.recordsHash, "frozen_records_pin");
  return { native_source: pins.native_source, sdk_source: pins.sdk_source, engine_build_id: pins.engine_build_id,
    server_build_id: pins.server_build_id, protocol_schema_sha256: pins.protocol_schema_sha256, bootstrap_schema_sha256: pins.bootstrap_schema_sha256,
    native_source_manifest_sha256: pins.native_source_manifest_sha256, sdk_build_manifest_sha256: sdkBuildPin,
    sdk_verification_sha256: sdkVerificationPin, sdk_source_bridge_sha256: pins.sdk_source_bridge_sha256 };
}
export async function freePort() {
  const server = createServer();
  await new Promise<void>((done, fail) => { server.once("error", fail); server.listen(0, "127.0.0.1", done); });
  const address = server.address(); check(address && typeof address !== "string", "fixture_port");
  await new Promise<void>(done => server.close(() => done()));
  return address.port;
}
export function privateTree(path: string) {
  for (const name of readdirSync(path)) {
    const file = join(path, name), info = lstatSync(file);
    check(!info.isSymbolicLink() && (info.mode & 0o777) === (info.isDirectory() ? 0o700 : 0o600), "private_evidence_mode");
    if (info.isDirectory()) privateTree(file);
  }
}
export async function startHostedFixture(options: { directory: string; dist?: string; inputs?: Inputs; humanThinkTimeMs?: number; stressRaisedLimits?: boolean }) {
  process.umask(0o077);
  mkdirSync(options.directory, { mode: 0o700 });
  check(options.humanThinkTimeMs === undefined || options.humanThinkTimeMs === 10_000, "fixture_think_time");
  const port = await freePort(), origin = `http://127.0.0.1:${port}`;
  const secret = randomBytes(32).toString("hex");
  Object.assign(process.env, {
    DECKBUILDER_MODE: "hosted", DECKBUILDER_PUBLIC_ORIGIN: origin,
    DECKBUILDER_DB: join(options.directory, "actor.sqlite"), DECKBUILDER_DATA: options.directory,
    DECKBUILDER_OPERATOR_NAME: "Disposable browser fixture", DECKBUILDER_CONTACT_EMAIL: "fixture@example.invalid",
    BETTER_AUTH_SECRET: secret, DISCORD_CLIENT_ID: "fixture", DISCORD_CLIENT_SECRET: "fixture",
    DECKBUILDER_INVITES_REQUIRED: "0",
  });
  for (const name of ["DECKBUILDER_CLAIM_CODE", "DECKBUILDER_TRUST_PROXY"]) delete process.env[name];
  // Resolve only after setting explicit disposable paths; db.ts reads env once.
  const { openDb } = await import("../../src/db.ts");
  const { createHostedAuth, provisionIdentity } = await import("../../src/auth.ts");
  const { LIMITS } = await import("../../src/ratelimit.ts");
  check(LIMITS.ui.count === 240 && LIMITS.ui.window === 60_000, "default_ui_budget");
  // Stress prefixes are explicit and cannot establish default-budget acceptance.
  if (options.stressRaisedLimits) for (const rule of Object.values(LIMITS) as { count: number }[]) rule.count = 1_000_000;
  const hostPath = createRequire(new URL("../../package.json", import.meta.url)).resolve("@mtg-ecosystem/deck-builder/host");
  const { createHost } = await import(hostPath) as typeof import("../../src/host.ts");
  const db = openDb(process.env.DECKBUILDER_DB!);
  let host: Awaited<ReturnType<typeof createHost>> | undefined;
  try {
    const auth = createHostedAuth(db), context = await auth.ready;
    async function enroll() {
      const id = randomUUID();
      const user = provisionIdentity(db, { id, name: `Fixture ${randomUUID()}` }, "");
      db.prepare('INSERT INTO user(id,name,email,emailVerified,createdAt,updatedAt) VALUES(?,?,?,1,?,?)')
        .run(id, "Disposable fixture", `${id}@example.invalid`, Date.now(), Date.now());
      const session = await context.internalAdapter.createSession(id); check(session, "fixture_session");
      const signature = createHmac("sha256", secret).update(session.token).digest("base64");
      return { user_id: user.user_id, cookie: encodeURIComponent(`${session.token}.${signature}`) };
    }
    const owner = await enroll(), foreign = await enroll();
    let native, identity: GameIdentity | undefined, deckId: number | undefined;
    let cards: { oracle_id: string; name: string }[] = [];
    if (options.inputs) {
      const pins = verifyInputs(options.inputs);
      const profile = JSON.parse(execFileSync(join(options.inputs.nativeRoot, "mtg-server"), ["lifecycle-profile", "commander-foundations-v1"], { encoding: "utf8", timeout: 10_000, stdio: ["ignore", "pipe", "ignore"] }));
      identity = { profile: profile.profile, catalog_sha256: profile.catalog_sha256, engine_build_id: profile.engine_build_id, server_build_id: profile.server_build_id };
      check(identity.profile === "commander-foundations-v1" && identity.engine_build_id === pins.engine_build_id && identity.server_build_id === pins.server_build_id, "profile_pin");
      const selected = profile.decks[0] as { commander: string; cards: { oracle_id: string; count: number }[] };
      check(selected.cards.reduce((sum, card) => sum + card.count, 0) === 100, "immutable_default_deck");
      // Accept the existing archive object or its JSONL representation. Ingest
      // exact records.source, never reconstructed card facts from compiled IR.
      const bytes = readFileSync(options.inputs.records, "utf8");
      let records;
      try { records = JSON.parse(bytes).records; } catch { records = bytes.trim().split("\n").map(line => JSON.parse(line)).filter(row => row.recordType === "card"); }
      check(Array.isArray(records), "frozen_records_shape");
      const sources = selected.cards.map(card => {
        const matches = records.filter(row => row.source?.oracle_id === card.oracle_id);
        check(matches.length === 1 && sha256(JSON.stringify(matches[0].source)) === matches[0].sourceSha256, "exact_frozen_record");
        return matches[0].source;
      });
      const { ingestCards } = await import("../../src/ingest.ts");
      cards = sources.map(card => ({ oracle_id: card.oracle_id, name: card.name }));
      const { createDeck } = await import("../../src/deck/service.ts");
      const { applyChanges } = await import("../../src/deck/membership.ts");
      check(ingestCards(db, sources).inserted === sources.length, "fixture_ingestion");
      const actor = { user_id: owner.user_id, via: "ui" as const, label: "ui" };
      deckId = createDeck(db, actor, "Native foundations default");
      applyChanges(db, actor, deckId, selected.cards.map(card => ({ action: "add", oracle_id: card.oracle_id, quantity: card.count, role: card.oracle_id === selected.commander ? "commander" : "card" })), { kind: "import" });
      const { createNativeLifecycle } = await import("../../../multiverse/src/native-lifecycle.ts");
      native = createNativeLifecycle({ executable: join(options.inputs.nativeRoot, "mtg-server"), directory: options.directory, identity, humanOrigin: origin });
    }
    host = await createHost({ mode: "hosted", port, databasePath: process.env.DECKBUILDER_DB, dataDirectory: options.directory, backgroundJobs: false,
      webDist: options.dist, isApplicationPath: (await import("../../../multiverse/src/page-paths.ts")).isApplicationPath,
      // The sole explicit test-only browser qualification. Default server stays false.
      games: native ? { native, browser_session_qualified: true, ...(options.humanThinkTimeMs ? { human_think_time_ms: options.humanThinkTimeMs } : {}) } : undefined });
    await host.start();
    return { origin, owner, foreign, deckId, identity, cards, uiRateBudget: LIMITS.ui.count, uiRateWindowMs: LIMITS.ui.window,
      capture: () => {
        const rows = db.prepare("SELECT record_json FROM game_launches WHERE user_id=?").all(owner.user_id) as { record_json: string }[];
        return rows.map(row => JSON.parse(row.record_json) as GameRecord);
      },
      async close() { try { await host!.close(); } finally { db.close(); } },
    };
  } catch (error) { try { await host?.close(); } finally { db.close(); } throw error; }
}
