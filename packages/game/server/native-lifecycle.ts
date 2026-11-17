import { spawn, type ChildProcess } from "node:child_process";
import { isAbsolute, join } from "node:path";
import { existsSync } from "node:fs";
import { timingSafeEqual } from "node:crypto";
import { producePresentation } from "@multiverse/game-server-sdk/presentation-producer";
import { GameClient, parseSeatBootstrap } from "@multiverse/game-server-sdk";
import type { GameIdentity, GameRecord, GameStopTarget, NativeGameAdapter, GameDiagnosticSink } from "../shared/contracts.ts";
import { nativeDiagnostics, nativeFailureCause, type NativeDiagnosticCause, type NativeFileDetail } from "./native-diagnostics.ts";
import { readNativeBootstrap } from "./native-bootstrap.ts";
import { nativeStatus } from "./native-status.ts";
import { createNativeAdmission } from "./native-admission.ts";
import { privateDirectory, readPrivate, writePrivate, type NativeFileObserver } from "./native-private.ts";
import { startBots, type BotGroup } from "./native-bot-supervisor.ts";

interface Running {
  child: ChildProcess;
  joined: Promise<void>;
  ready: Promise<void>;
  origin?: string;
  session?: string;
  bots?: BotGroup;
  presentation?: { descriptor: Awaited<ReturnType<typeof producePresentation>>["descriptor"]; body: string };
  stopping: boolean;
  binding: string;
}
export interface NativeLifecycleOptions { executable: string; identity: GameIdentity; directory: string; humanOrigin: string; diagnostic?: GameDiagnosticSink }
const same = (a: GameIdentity, b: GameIdentity) => ["catalog_sha256", "profile", "engine_build_id", "server_build_id"].every(key => a[key as keyof GameIdentity] === b[key as keyof GameIdentity]);
const unavailable = () => new Error("Native lifecycle unavailable");
/** Actual offline registry/native process bridge. Default browser qualification
 * remains false in composition. Only foundations' existing finite policy runs;
 * attachments remain separately admitted but unavailable for automated launch. */
export function createNativeLifecycle(options: NativeLifecycleOptions): NativeGameAdapter {
  if (!isAbsolute(options.executable) || !isAbsolute(options.directory)) throw new Error("Native lifecycle requires absolute paths");
  const publicOrigin = new URL(options.humanOrigin);
  if (publicOrigin.protocol !== "http:" || !["127.0.0.1", "[::1]"].includes(publicOrigin.hostname) || `http://${publicOrigin.hostname}:${publicOrigin.port || "80"}` !== options.humanOrigin) throw new Error("Native human origin must be configured numeric loopback");
  privateDirectory(options.directory);
  const diagnostic = nativeDiagnostics(options.diagnostic);
  const admission = createNativeAdmission(options.executable, options.identity);
  const running = new Map<string, Running>();
  const stoppingGames = new Set<string>();
  const nativeEnvironment = (): NodeJS.ProcessEnv => ({ PATH: "/usr/bin:/bin", ...(process.env.XDG_RUNTIME_DIR ? { XDG_RUNTIME_DIR: process.env.XDG_RUNTIME_DIR } : {}), ...(process.env.DBUS_SESSION_BUS_ADDRESS ? { DBUS_SESSION_BUS_ADDRESS: process.env.DBUS_SESSION_BUS_ADDRESS } : {}), ...(process.env.LD_LIBRARY_PATH ? { LD_LIBRARY_PATH: process.env.LD_LIBRARY_PATH } : {}) });
  const joinProcess = (command: string, args: string[]) => new Promise<void>((resolve, reject) => {
    const child = spawn(command, args, { stdio: "ignore", env: nativeEnvironment() });
    child.once("error", () => reject(unavailable())); child.once("close", code => code === 0 ? resolve() : reject(unavailable()));
  });
  const root = (game: GameStopTarget) => {
    if (!/^[a-f0-9]{32}$/.test(game.id)) throw unavailable();
    return join(options.directory, game.id);
  };
  const bootstrap = (game: GameRecord, observe?: NativeFileObserver) =>
    readNativeBootstrap(game.identity, options.identity, () => running.get(game.id), () => root(game), observe);
  const validate = (bytes: string, game: GameRecord) => {
    try {
      const seat = parseSeatBootstrap(bytes);
      const authorized = parseSeatBootstrap(bootstrap(game));
      if (!timingSafeEqual(Buffer.from(seat.capability), Buffer.from(authorized.capability))) return false;
      return seat.viewer === 0 && seat.match_id === game.id && seat.server_origin === options.humanOrigin && seat.engine_build_id === game.identity.engine_build_id && seat.server_build_id === game.identity.server_build_id && seat.catalog_sha256 === game.identity.catalog_sha256;
    } catch { return false; }
  };
  const generation = (directory: string): number => {
    const path = join(directory, "generation.json");
    const value: unknown = existsSync(path) ? JSON.parse(readPrivate(path)) : 0;
    if (!Number.isSafeInteger(value) || (value as number) < 0) throw unavailable();
    return value as number;
  };
  const stopRunning = async (state: Running) => {
    state.stopping = true;
    state.child.stdin?.end();
    await state.bots?.stop();
    await state.joined;
  };
  return {
    identity: { ...options.identity },
    // Focused native lifecycle proof is retained in game-client evidence. The
    // default entrypoint independently keeps browser session qualification off.
    lifecycle_qualified: true,
    async admit(deck, signal) {
      const result = await admission.admit(deck, signal);
      if (result.state === "supported" && options.identity.profile !== "commander-foundations-v1") return { state: "unavailable", reason: "session_incomplete" };
      return result;
    },
    async ensure(game, signal) {
      let cause: NativeDiagnosticCause = "identity_failure", reported = false;
      let fileDetail: NativeFileDetail | undefined;
      try {
        signal.throwIfAborted();
        if (!same(game.identity, options.identity)) throw unavailable();
        cause = "state_unavailable";
        if (stoppingGames.has(game.id)) throw unavailable();
        cause = "private_file";
        const directory = root(game); privateDirectory(directory);
        const epoch = generation(directory);
        const immutableBinding = JSON.stringify([game.deck, game.identity, options.humanOrigin]);
        cause = "state_unavailable";
        const previous = running.get(game.id);
        if (previous) { if (previous.binding !== immutableBinding || previous.stopping) throw unavailable(); await previous.ready; signal.throwIfAborted(); return; }
        if (running.size >= 2) throw unavailable();
        const commanders = game.deck.cards.filter(c => c.role === "commander");
        if (commanders.length !== 1 || game.deck.cards.some(c => c.role === "companion") || game.identity.profile !== "commander-foundations-v1") throw unavailable();
        cause = "startup_failure";
        const child = spawn(options.executable, ["lifecycle", directory], { stdio: ["pipe", "pipe", "ignore"], env: nativeEnvironment() });
        child.stdin!.on("error", () => {});
        const joined = new Promise<void>(resolve => { child.once("close", () => resolve()); child.once("error", () => resolve()); });
        const state: Running = { child, joined, ready: Promise.resolve(), stopping: false, binding: immutableBinding };
        running.set(game.id, state);
        child.once("exit", () => {
          diagnostic(game.id, "child", state.stopping ? "child_exit_during_stop" : "child_exit");
          state.stopping = true; void state.bots?.stop();
        });
        child.once("error", () => diagnostic(game.id, "child", "child_error"));
        const abort = () => { void stopRunning(state); };
        signal.addEventListener("abort", abort, { once: true });
        state.ready = (async () => {
          try {
            const ready = await new Promise<{ origin: string; session: string }>((resolve, reject) => {
              let output = "";
              child.stdout!.on("data", chunk => {
                output += chunk.toString();
                if (output.length > 4096) { cause = "startup_protocol"; reject(unavailable()); child.stdin!.end(); return; }
                if (!output.endsWith("\n")) return;
                try {
                  const value = JSON.parse(output);
                  if (value.version !== 1 || !/^session-[a-f0-9]{32}$/.test(value.session) || !/^http:\/\/127\.0\.0\.1:[1-9][0-9]{0,4}$/.test(value.origin)) throw unavailable();
                  resolve(value);
                } catch { cause = "startup_protocol"; reject(unavailable()); }
              });
              child.once("exit", () => reject(unavailable())); child.once("error", () => reject(unavailable()));
            });
            if (signal.aborted || state.stopping || generation(directory) !== epoch) throw unavailable();
            state.origin = ready.origin; state.session = ready.session;
            cause = "private_file";
            if (!validate(bootstrap(game, detail => { fileDetail = detail; }), game)) { cause = "identity_failure"; throw unavailable(); }
            const human = parseSeatBootstrap(bootstrap(game, detail => { fileDetail = detail; }));
            const sdk = new GameClient({ baseURL: state.origin, matchId: game.id, capability: human.capability });
            cause = "adapter_failure";
            const manifest = await sdk.manifest({ signal });
            cause = "presentation_failure";
            const produced = await producePresentation(manifest, { match_id: game.id, ...game.identity });
            if (signal.aborted || state.stopping || generation(directory) !== epoch || !same(game.identity, options.identity)) throw unavailable();
            state.presentation = { descriptor: produced.descriptor, body: new TextDecoder("utf-8", { fatal: true }).decode(produced.bytes) };
            cause = "bots_failure";
            state.bots = await startBots(directory, ready.session, game.identity.profile, options.humanOrigin, signal);
            if (signal.aborted || state.stopping || generation(directory) !== epoch) throw unavailable();
          } catch (error) {
            reported = true;
            diagnostic(game.id, "ensure", nativeFailureCause(error, signal, cause), undefined, fileDetail);
            diagnostic(game.id, "ensure_cleanup", "cleanup_started");
            try { await stopRunning(state); diagnostic(game.id, "ensure_cleanup", "cleanup_succeeded"); }
            catch (cleanupError) { diagnostic(game.id, "ensure_cleanup", "cleanup_failed"); throw cleanupError; }
            throw unavailable();
          }
          finally { signal.removeEventListener("abort", abort); }
        })();
        child.stdin!.write(JSON.stringify({ version: 1, generation: epoch, id: game.id, ...game.identity, deck_sha256: game.deck.sha256, human_origin: options.humanOrigin, deck: { commander: commanders[0]!.oracle_id, cards: game.deck.cards.map(c => ({ oracle_id: c.oracle_id, count: c.quantity })) } }) + "\n");
        await state.ready;
      } catch (error) {
        if (!reported) diagnostic(game.id, "ensure", nativeFailureCause(error, signal, cause), undefined, fileDetail);
        throw error;
      }
    },
    async status(game, signal) {
      return nativeStatus(game, signal, { identity: options.identity, state: running.get(game.id), bootstrap: observe => bootstrap(game, observe),
        readStatus: observe => readPrivate(join(root(game), "status.json"), 65_536, observe), diagnostic });
    },
    async stop(game, reason, signal) {
      let cause: NativeDiagnosticCause = "private_file";
      try {
        const directory = root(game); privateDirectory(directory);
        const bindingPath = join(directory, "binding.json");
        if (existsSync(bindingPath)) {
          const bound = JSON.parse(readPrivate(bindingPath)) as GameIdentity & { id: string };
          if (bound.id !== game.id || !same(bound, game.identity)) { cause = "identity_failure"; throw unavailable(); }
        }
        stoppingGames.add(game.id);
        // Stop can fence an older exact retained binding without loading its rules
        // or treating the current executable as a recovery-compatible build.
        // Capture-before-await generations fence late/uncertain ensure, while a
        // later explicit recovery may own a fresh generation of this same match.
        if (reason !== "shutdown") {
          const next = generation(directory) + 1;
          if (!Number.isSafeInteger(next)) throw unavailable();
          writePrivate(join(directory, "generation.json"), JSON.stringify(next));
        }
        cause = "adapter_failure";
        const state = running.get(game.id);
        if (state) {
          await stopRunning(state);
          await state.ready.catch(() => {});
          await state.bots?.stop();
        }
        // The exact persisted native unit and all independent kernel bot leases
        // must be quiescent even when ensure's response/process ownership was lost.
        await joinProcess(options.executable, ["lifecycle-join", directory]);
        for (const seat of [1, 2, 3]) {
          const lease = join(directory, `bot-${seat}`, "lease");
          if (existsSync(lease)) { readPrivate(lease); await joinProcess("flock", ["--exclusive", lease, "/usr/bin/true"]); }
        }
        running.delete(game.id);
        stoppingGames.delete(game.id);
        signal.throwIfAborted();
      } catch (error) { diagnostic(game.id, "stop", nativeFailureCause(error, signal, cause)); throw error; }
    },
    async proxy(game, request, signal) {
      const state = running.get(game.id);
      signal.throwIfAborted();
      if (!same(game.identity, options.identity)) throw unavailable();
      if (!state?.origin || state.stopping || state.child.exitCode !== null || state.child.signalCode !== null || !state.bots || state.bots.status().some(s => s !== "running")) throw unavailable();
      if (Buffer.byteLength(request.body) > 65_536 || (request.method === "GET" && request.body)) throw unavailable();
      if (request.method === "GET" && state.presentation && request.path === state.presentation.descriptor.path) return { status: 200, body: state.presentation.body };
      if (request.method === "GET" && request.path === "/client-config.json") return { status: 200, body: JSON.stringify({ client_config_version: 1, server_origin: options.humanOrigin }) };
      const human = parseSeatBootstrap(bootstrap(game));
      const expected = Buffer.from(`Bearer ${human.capability}`);
      const received = Buffer.from(request.authorization);
      if (expected.length !== received.length || !timingSafeEqual(expected, received)) throw unavailable();
      if (request.method === "GET" && request.path === `/v1/matches/${game.id}/presentation`) {
        if (!state.presentation) throw unavailable();
        return { status: 200, body: JSON.stringify(state.presentation.descriptor) };
      }
      const url = new URL(request.path, state.origin);
      if (url.origin !== state.origin || url.pathname !== request.path.split("?")[0] || request.path.includes("%") || request.path.includes("\\")) throw unavailable();
      const prefix = `/v1/matches/${game.id}/`;
      if (!url.pathname.startsWith(prefix)) throw unavailable();
      const tail = url.pathname.slice(prefix.length);
      // Reviewed route authority: game-server/src/protocol.rs ROUTES. No arbitrary
      // proxy, operator, other match, encoded path or caller-selected destination.
      const allowed = request.method === "GET" ? /^(manifest|resume|presentation|events|commands\/[a-f0-9]{32})$/.test(tail) : /^(commands|actions\/query|commands\/[a-f0-9]{32}\/cancel)$/.test(tail);
      if (!allowed || (url.search && !(request.method === "GET" && tail === "events"))) throw unavailable();
      if (url.search) {
        const keys = [...url.searchParams.keys()];
        if (new Set(keys).size !== keys.length || keys.some(k => !["after", "limit", "wait_ms"].includes(k))) throw unavailable();
      }
      const response = await fetch(url, { method: request.method, headers: { Authorization: request.authorization, Host: new URL(state.origin).host, Origin: state.origin, Accept: "application/json", "Content-Type": "application/json" }, ...(request.method === "POST" ? { body: request.body } : {}), redirect: "error", signal });
      const declared = Number(response.headers.get("content-length"));
      if (response.redirected || !/^application\/json(?:\s*;|$)/i.test(response.headers.get("content-type") ?? "") || Number.isFinite(declared) && declared > 8 * 1024 * 1024) {
        await response.body?.cancel().catch(() => {}); throw unavailable();
      }
      const reader = response.body?.getReader();
      if (!reader) throw unavailable();
      const chunks: Uint8Array[] = []; let size = 0;
      try {
        for (;;) { const chunk = await reader.read(); if (chunk.done) break; size += chunk.value.length; if (size > 8 * 1024 * 1024) throw unavailable(); chunks.push(chunk.value); }
        signal.throwIfAborted();
        if (state.stopping || running.get(game.id) !== state || !same(game.identity, options.identity)) throw unavailable();
        return { status: response.status, body: new TextDecoder("utf-8", { fatal: true }).decode(Buffer.concat(chunks)) };
      } finally { await reader.cancel().catch(() => {}); }
    },
    async humanBootstrap(game, signal) { signal.throwIfAborted(); const bytes = bootstrap(game); if (!validate(bytes, game)) throw unavailable(); return bytes; },
    validateHumanBootstrap: validate,
  };
}

/** All existing admission pins plus two explicit host-private lifecycle settings.
 * No environment variable can qualify browser controls/session composition. */
export function nativeLifecycleFromEnvironment(env: NodeJS.ProcessEnv = process.env): NativeGameAdapter | undefined {
  const directory = env.MULTIVERSE_NATIVE_LIFECYCLE_DIRECTORY;
  const humanOrigin = env.MULTIVERSE_NATIVE_HUMAN_ORIGIN;
  if (!directory && !humanOrigin) return undefined;
  const executable = env.MULTIVERSE_NATIVE_EXECUTABLE;
  const catalog = env.MULTIVERSE_NATIVE_CATALOG_SHA256;
  const profile = env.MULTIVERSE_NATIVE_PROFILE;
  const engine = env.MULTIVERSE_NATIVE_ENGINE_BUILD_ID;
  const server = env.MULTIVERSE_NATIVE_SERVER_BUILD_ID;
  if (!directory || !humanOrigin || !executable || !catalog || !profile || !engine || !server) throw new Error("Native lifecycle configuration is incomplete");
  return createNativeLifecycle({ directory, humanOrigin, executable, identity: { catalog_sha256: catalog, profile, engine_build_id: engine, server_build_id: server } });
}
