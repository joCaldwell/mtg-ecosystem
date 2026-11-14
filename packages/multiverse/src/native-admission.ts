import { spawn } from "node:child_process";
import { isAbsolute } from "node:path";
import type { GameAdmission, GameDeck, GameIdentity, NativeGameAdapter } from "@mtg-ecosystem/deck-builder/contracts";

const unavailable = (): GameAdmission => ({ state: "unavailable", reason: "native_unavailable" });
/** Admission-only real native CLI adapter. No shell, inherited secrets, catalog
 * paths, seeds, registry mutation, or pretend provisioning. Rust admit-client
 * is authoritative for selected reviewed profile + exact embedded catalog/pod. */
export function createNativeAdmission(executable: string, identity: GameIdentity): NativeGameAdapter {
  if (!isAbsolute(executable)) throw new Error("Native executable must be an absolute path");
  const pins = { catalog_sha256: identity.catalog_sha256, profile: identity.profile, engine_build_id: identity.engine_build_id, server_build_id: identity.server_build_id };
  return {
    identity: pins,
    lifecycle_qualified: false,
    async admit(deck: GameDeck, signal: AbortSignal): Promise<GameAdmission> {
      if (deck.cards.some(card => card.role === "companion")) return { state: "unsupported", code: "companion" };
      const commanders = deck.cards.filter(card => card.role === "commander");
      if (commanders.length > 1) return { state: "unsupported", code: "multiple_commanders" };
      if (commanders.length !== 1 || commanders[0].quantity !== 1) return { state: "unsupported", code: "deck" };
      const nativeDeck = { commander: commanders[0].oracle_id, cards: deck.cards.map(card => ({ oracle_id: card.oracle_id, count: card.quantity })) };
      const input = JSON.stringify({ version: 1, operation: "admit", profile: pins.profile, engine_build_id: pins.engine_build_id, server_build_id: pins.server_build_id, catalog_sha256: pins.catalog_sha256, deck: nativeDeck });
      if (Buffer.byteLength(input) > 65_536 || signal.aborted) return unavailable();
      return new Promise<GameAdmission>(resolve => {
        let child: ReturnType<typeof spawn>;
        try { child = spawn(executable, ["admit-client"], { stdio: ["pipe", "pipe", "ignore"], env: { ...(process.env.LD_LIBRARY_PATH ? { LD_LIBRARY_PATH: process.env.LD_LIBRARY_PATH } : {}) }, signal, timeout: 10_000, killSignal: "SIGKILL" }); }
        catch { resolve(unavailable()); return; }
        const chunks: Buffer[] = []; let size = 0, overflow = false;
        child.stdout!.on("data", (chunk: Buffer) => {
          size += chunk.length;
          if (size > 65_536) { overflow = true; child.kill("SIGKILL"); } else chunks.push(chunk);
        });
        child.stdin!.on("error", () => {});
        child.on("error", () => resolve(unavailable()));
        child.on("close", code => {
          if (code !== 0 || overflow || signal.aborted) { resolve(unavailable()); return; }
          try {
            const answer = JSON.parse(Buffer.concat(chunks).toString("utf8"));
            if (answer.version !== 1) { resolve(unavailable()); return; }
            if (answer.status === "rejected") {
              resolve(answer.reason === "identity" ? { state: "unavailable", reason: "identity_mismatch" } : answer.reason === "profile" ? { state: "unsupported", code: "catalog" } : answer.reason === "deck" ? { state: "unsupported", code: "deck" } : unavailable());
              return;
            }
            if (answer.status !== "admitted" || answer.profile !== pins.profile || answer.engine_build_id !== pins.engine_build_id || answer.server_build_id !== pins.server_build_id || answer.catalog_sha256 !== pins.catalog_sha256 || !sameDeck(answer.deck, nativeDeck)) { resolve({ state: "unavailable", reason: "identity_mismatch" }); return; }
            resolve({ state: "supported", identity: pins, deck_sha256: deck.sha256, launch_ready: false });
          } catch { resolve(unavailable()); }
        });
        child.stdin!.end(input);
      });
    },
    async ensure() { throw new Error("Native provisioning bridge is not qualified"); },
    async status() { throw new Error("Native consumer supervision is not qualified"); },
    async stop() { throw new Error("Native consumer supervision is not qualified"); },
    async humanBootstrap() { throw new Error("Native human session delivery is not qualified"); },
    validateHumanBootstrap() { return false; },
  };
}
function sameDeck(value: unknown, expected: { commander: string; cards: { oracle_id: string; count: number }[] }): boolean {
  if (!value || typeof value !== "object" || !("commander" in value) || value.commander !== expected.commander || !("cards" in value) || !Array.isArray(value.cards) || value.cards.length !== expected.cards.length) return false;
  const cards = value.cards as unknown[];
  const seen = new Set<string>();
  return cards.every(card => {
    if (!card || typeof card !== "object" || !("oracle_id" in card) || typeof card.oracle_id !== "string" || !("count" in card) || seen.has(card.oracle_id)) return false;
    seen.add(card.oracle_id);
    return expected.cards.some(c => c.oracle_id === card.oracle_id && c.count === card.count);
  });
}
/** Configuration is opt-in and all-or-nothing. There is deliberately no env
 * switch that claims lifecycle or browser decision qualification. */
export function nativeAdmissionFromEnvironment(env: NodeJS.ProcessEnv = process.env): NativeGameAdapter | undefined {
  const keys = ["MULTIVERSE_NATIVE_EXECUTABLE", "MULTIVERSE_NATIVE_CATALOG_SHA256", "MULTIVERSE_NATIVE_PROFILE", "MULTIVERSE_NATIVE_ENGINE_BUILD_ID", "MULTIVERSE_NATIVE_SERVER_BUILD_ID"] as const;
  if (keys.every(key => env[key] === undefined)) return undefined;
  if (keys.some(key => !env[key])) throw new Error("Native admission configuration is incomplete");
  const [, catalog, profile, engine, server] = keys.map(key => env[key]!);
  if (!/^[a-f0-9]{64}$/.test(catalog) || [profile, engine, server].some(v => v.length > 256)) throw new Error("Native admission configuration is invalid");
  return createNativeAdmission(env.MULTIVERSE_NATIVE_EXECUTABLE!, { catalog_sha256: catalog, profile, engine_build_id: engine, server_build_id: server });
}
