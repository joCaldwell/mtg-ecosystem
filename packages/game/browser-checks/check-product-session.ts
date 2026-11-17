#!/usr/bin/env node
// Opt-in actual built-product acceptance. No Cargo, SDK generation, fake native
// responses, human SDK policy or second human reader. Every command is UI-owned.
import { execFile, fork, spawnSync } from "node:child_process";
import { existsSync, readdirSync, mkdirSync, mkdtempSync, openSync, closeSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, firefox, type Page, type BrowserContext, type Request } from "playwright-core";
import type { GameRecord } from "../shared/contracts.ts";
import type { ActionBeginning, ActionInitiation, Observation, PlayerResponse, SeatBootstrap, RulesResult } from "@multiverse/game-server-sdk";
import { check, completeResponse, fileHash, repository, verifyInputs, OBSERVATION_VERSION, type Inputs } from "./lib/product-session-fixture.ts";
import { ObserverAudit, SubmissionAudit, finishOwnedChild, discoverWithRateRecovery, waitForUiRead, readCooldownPending, boundedRetryAfter, queryRateInvalidated } from "./lib/product-session-safety.ts";

const option = (name: string) => { const i = process.argv.indexOf(`--${name}`); return i < 0 ? undefined : process.argv[i + 1]; };
const required = (name: string) => { const value = option(name); check(value && !value.startsWith("--"), `argument_${name}`); return resolve(value); };
const delay = (ms: number) => new Promise<void>(done => setTimeout(done, ms));
const shutdown = new AbortController();
process.once("SIGINT", () => shutdown.abort());
process.once("SIGTERM", () => shutdown.abort());
process.once("SIGHUP", () => shutdown.abort());
async function until(predicate: () => unknown | Promise<unknown>, code: string, ms = 20_000) {
  const end = Date.now() + ms;
  while (Date.now() < end) { check(!shutdown.signal.aborted, "check_aborted"); if (await predicate()) return; await delay(30); }
  throw Error(code);
}
async function bounded<T>(work: Promise<T>, ms: number, code: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try { return await Promise.race([work, new Promise<never>((_, fail) => { timer = setTimeout(() => fail(Error(code)), ms); })]); }
  finally { clearTimeout(timer); }
}
function initiationResponse(action: ActionInitiation): PlayerResponse {
  switch (action.action) {
    case "pass": return { response: "pass" };
    case "play_land": return { response: "play_land", object: action.object };
    case "cast": return { response: "cast", card: action.card };
    case "activate_mana": return { response: "activate_mana", object: action.object };
    case "activate_ability": return { response: "activate_ability", object: action.object, ability: action.ability };
    case "finish_mana": return { response: "finish_mana", undo_mana: action.undo_mana };
  }
}
async function journal(page: Page, exactBody?: string, binding?: Record<string, string>) {
  return page.evaluate(async ({ body, binding }) => {
    const db = await new Promise<IDBDatabase>((done, fail) => {
      const open = indexedDB.open("multiverse-command-journal", 1);
      open.onsuccess = () => done(open.result); open.onerror = () => fail(Error("journal_open"));
    });
    try {
      return await new Promise<boolean>((done, fail) => {
        const tx = db.transaction("commands", "readonly"), read = tx.objectStore("commands").getAll();
        // Returning after transaction complete also proves the preceding strict
        // write committed; no actual private record leaves the browser.
        tx.oncomplete = () => done(body === undefined ? read.result.length === 0 : read.result.length === 1
          && read.result[0].version === 1 && read.result[0].exactBody === body
          && Object.keys(read.result[0]).sort().join() === "binding,exactBody,version"
          && binding !== undefined && Object.keys(read.result[0].binding).length === Object.keys(binding).length
          && Object.entries(binding).every(([key, value]) => read.result[0].binding[key] === value));
        tx.onerror = tx.onabort = () => fail(Error("journal_read"));
      });
    } finally { db.close(); }
  }, { body: exactBody, binding });
}
interface Ready {
  ready: true; origin: string; owner: { user_id: number; cookie: string }; foreign: { user_id: number; cookie: string };
  deckId: number; identity: { engine_build_id: string; server_build_id: string; catalog_sha256: string; profile: string };
  cards: { oracle_id: string; name: string }[]; uiRateBudget: number; uiRateWindowMs: number;
}
type RecordedOutcome = { kind: "win"; winner: number } | { kind: "draw" };

async function run(name: "chromium" | "firefox", output: string, dist: string, inputs: Inputs, natural: boolean, pauseCase: boolean) {
  const pins = verifyInputs(inputs);
  const naturalLimitMs = natural ? Number(option("natural-ms") ?? 240_000) : 0;
  check(!natural || Number.isSafeInteger(naturalLimitMs) && naturalLimitMs >= 1 && naturalLimitMs <= 5_400_000, "natural_bound");
  const checkerSources = ["check-product-session.ts", "lib/product-session-fixture.ts", "lib/product-session-host.ts", "lib/product-session-safety.ts"].map(path => ({ path, sha256: fileHash(fileURLToPath(new URL(path, import.meta.url))) }));
  const directory = mkdtempSync(join(output, `${name}-`));
  const log = openSync(join(directory, "host.log"), "wx", 0o600);
  const child = fork(fileURLToPath(new URL("./lib/product-session-host.ts", import.meta.url)), [], { stdio: ["ignore", log, log, "ipc"] });
  closeSync(log);
  let stopped = false, joinedLeases = 0;
  child.on("message", (value: any) => { if (value.stopped) { stopped = true; joinedLeases = value.leases; } });
  const readiness = bounded(new Promise<Ready>((done, fail) => {
    child.once("error", () => fail(Error("fixture_process")));
    child.once("exit", () => fail(Error("fixture_early_exit")));
    child.once("message", (value: any) => value.ready ? done(value) : fail(Error("fixture_start")));
  }), 170_000, "fixture_start_deadline");
  const stressRaisedLimits = process.argv.includes("--stress-raised-limits");
  child.send({ directory: join(directory, "private"), dist, inputs, stressRaisedLimits, ...(pauseCase ? { humanThinkTimeMs: 10_000 } : {}) });
  let browser, context: BrowserContext | undefined, page: Page | undefined, ready: Ready | undefined;
  const audit = new ObserverAudit();
  let failure = "", presentationMissing = 0, presentationVerified = false, presentationArtifacts = 0, namedCards = 0;
  let immutableAssetsRevalidated = false;
  let observation: Observation | null = null, revision = "", outcome: RulesResult | null = null, lifecycle = "";
  let gameId = "", artifactPath = "", bootstrap: SeatBootstrap | undefined, grants = 0, nativeReads = 0, activeReads = 0;
  let lazyLoaded = false, admission = false, launched = false, recovered = false, unmounted = false, regranted = false;
  let negatives = 0, conditionalReversal = false, commanderResolved = false, landPlayed = false;
  let naturalAttempted = false, naturalCompleted = false, naturalPaused = false, naturalBounded = false, naturalUnsupported = false;
  let recordOutcome: RecordedOutcome | null = null;
  let recordCompleted = false, recordPaused = false, uiOutcomeMatches = false, journalUnresolved = false, recordReads = 0;
  let recordPauseCode = "", resourcePauseVerified = false, commandControlsDisabled = false;
  let recordState = "", pauseBannerVisible = false, interruptionBannerVisible = false, pauseCauseVisible = false;
  let naturalPauseUiObserved = false, naturalPauseCauseVisible = false, naturalPauseControls = 0;
  const recordPauseCounts: Record<string, number> = {};
  const recordStateCounts: Record<string, number> = {};
  let beginnings: ActionBeginning[] = [], requests = 0, queries = 0;
  type Discovery = { context: { revision: string; token: string }; beginnings: ActionBeginning[]; calls: number; nativeStale: boolean; lastHttp: number; rateHint: boolean };
  let discovery: Discovery | null = null;
  const queryOwners = new WeakMap<Request, Discovery>();
  const readOrdinals = new WeakMap<Request, number>();
  let readOrdinal = 0, successfulReadOrdinal = 0, queryRateReadFloor = -1, queryRateLimits = 0, queryRateInvalidations = 0;
  const queryCooldownPending = () => readCooldownPending(queryRateLimits, queryRateReadFloor, successfulReadOrdinal);
  let readRateLimits = 0, resumesAtRateLimit = 0, recoveredCommands = 0, naturalEnd = Infinity;
  let readCooldownWaits = 0, readCooldownReady = 0;
  let invalidatedCompleteMenus = 0, lateNoSendInvalidations = 0, actionClickTimeouts = 0;
  let retryAfterPresent = 0, retryAfterMissing = 0, retryAfterUnbounded = 0;
  const retryAfterSeconds: Record<string, number> = {};
  const rateRecovery = { handled: 0, started: 0, ready: 0, freshDiscoveries: 0, retired: 0 };
  const staleRediscovery = { native: 0, local: 0, started: 0, ready: 0, freshGenerations: 0,
    contextChanges: 0, tokenChanges: 0, cursorChanges: 0, maxClicks: 0, exhausted: 0 };
  let discoveryGenerations = 0, staleRecoveredCommands = 0;
  let pending: { response: PlayerResponse; revision: string; token: string } | undefined;
  type SavedIntent = { response: PlayerResponse; revision: string; token: string; binding: Record<string, string> };
  const submissions = new SubmissionAudit<SavedIntent>();
  const submissionRequestIds = new WeakMap<Request, string>();
  const submissionObservations = new Map<string, Promise<void>>();
  let submissionFailure = "", submissionRetryClicks = 0, submissionRetryTimeouts = 0;
  const sent = new Map<string, string>(), accepted = new Set<string>(), consumed = new Set<string>();
  const requestKinds: Record<string, number> = {}, responseKinds: Record<string, number> = {};
  const responseAttemptKinds: Record<string, number> = {};
  const nativeStatuses: Record<string, number> = {};
  const httpStatuses: Record<string, number> = {}, recordHttpStatuses: Record<string, number> = {};
  const queryOutcomes: Record<string, number> = {};
  const discoveryPhases: Record<string, number> = {};
  let lastDiscoveryCalls = 0, lastDiscoveryBeginnings = 0, lastDiscoveryPassOffered = false;
  let journalBeforeSend = 0, originalContexts = 0, completeResponses = 0, authenticatedPosts = 0;
  const add = (counts: Record<string, number>, name: string) => { counts[name] = (counts[name] ?? 0) + 1; };
  const connected = () => page!.getByRole("heading", { name: "Connected · Seat 1", exact: true }).isVisible();
  const inspectPauseUI = async () => {
    check(recordState === "paused" || recordState === "interrupted", "inactive_record_required");
    const title = recordState === "paused" ? "Game paused" : "Game interrupted";
    let controls = 0, commandsBlocked = false;
    await until(async () => {
      if (!await page!.getByRole("heading", { name: title, exact: true }).isVisible()) return false;
      // Scope selection/inspection and stopping read-only discovery are not
      // game commands. Check every exposed dispatch control, not just opening.
      const buttons = page!.getByRole("button", { name: /^(Confirm choice|Discover actions|Check or retry saved choice)$/ })
        .or(page!.getByRole("region", { name: "Available actions", exact: true }).locator("ol > li > button"));
      controls = await buttons.count(); commandsBlocked = true;
      for (let i = 0; i < controls; i++) commandsBlocked &&= await buttons.nth(i).isDisabled() || await buttons.nth(i).getAttribute("aria-disabled") === "true";
      return commandsBlocked;
    }, "pause_ui_commands_blocked");
    check(recordOutcome === null && !outcome
      && await page!.getByRole("heading", { name: "Game complete", exact: true }).count() === 0
      && await page!.getByText(/^Rules outcome:/).count() === 0
      && await page!.getByText("You won.", { exact: true }).count() === 0
      && await page!.getByText("The game ended in a draw.", { exact: true }).count() === 0, "pause_is_not_rules_outcome");
    const cause = recordPauseCode === "human_timeout" ? "The game stopped after the human decision time limit. Its saved record is retained."
      : recordPauseCode === "lifetime_exceeded" ? "The game stopped after the game time limit. Its saved record is retained." : null;
    const causeVisible = cause !== null && await page!.getByRole("status").filter({ hasText: cause }).isVisible();
    if (cause) check(causeVisible, "pause_cause_visible");
    return { controls, commandsBlocked, causeVisible };
  };
  const untilRead = async (predicate: () => Promise<boolean>, code: string) => {
    const extended = await waitForUiRead({ end: naturalEnd, code, now: Date.now, predicate, wait: () => delay(30),
      ratePending: () => naturalAttempted && (readCooldownPending(readRateLimits, resumesAtRateLimit, activeReads) || queryCooldownPending() || submissions.pendingRateLimited), onCooldown: () => { readCooldownWaits++; },
      healthy: () => {
        check(!shutdown.signal.aborted, "check_aborted");
        check(!audit.protocolFailed && audit.pageErrors === 0, "rate_recovery_protocol_or_page_error");
        check(!submissionFailure, submissionFailure);
      },
    });
    if (extended && activeReads > resumesAtRateLimit && await connected()) readCooldownReady++;
  };
  const installRecord = (game: GameRecord) => {
    recordReads++;
    check(game.id === gameId && game.identity.engine_build_id === ready!.identity.engine_build_id
      && game.identity.server_build_id === ready!.identity.server_build_id && game.identity.catalog_sha256 === ready!.identity.catalog_sha256
      && game.identity.profile === ready!.identity.profile, "outcome_record_identity");
    check(["provisioning", "active", "paused", "completed", "interrupted"].includes(game.state), "typed_record_state");
    recordState = game.state; add(recordStateCounts, recordState);
    recordCompleted = game.state === "completed";
    recordPaused = ["paused", "interrupted"].includes(game.state);
    check(game.pause === null || ["consumer_failed", "human_timeout", "native_unavailable", "identity_mismatch", "lifetime_exceeded"].includes(game.pause), "typed_record_pause");
    recordPauseCode = game.pause ?? "";
    if (recordPauseCode) add(recordPauseCounts, recordPauseCode);
    check(game.outcome === null || game.outcome?.kind === "draw"
      || (game.outcome?.kind === "win" && [0, 1, 2, 3].includes(game.outcome.winner)), "typed_record_outcome");
    recordOutcome = game.outcome;
  };
  try {
    ready = await readiness;
    const sdk = await import("@multiverse/game-server-sdk");
    // The harness owns shutdown; Playwright's SIGINT handler can exit before finally.
    browser = await ({ chromium, firefox }[name]).launch({ headless: true, handleSIGINT: false, handleSIGTERM: false, handleSIGHUP: false });
    context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, serviceWorkers: "block", reducedMotion: "reduce" });
    const cookie = (value: string) => ({ name: "better-auth.session_token", value, url: ready!.origin, httpOnly: true, sameSite: "Lax" as const });
    await context.addCookies([cookie(ready.owner.cookie)]);
    page = await context.newPage(); page.setDefaultTimeout(20_000);
    page.on("pageerror", () => { audit.pageErrors++; });
    // No traces, HAR, video, screenshots, console text, DOM snapshots or failing
    // locator diagnostics are captured. Plain playwright-core avoids test-runner
    // automatic authenticated failure artifacts.
    page.on("response", response => {
      const job = audit.track(async () => {
        const path = new URL(response.url()).pathname;
        add(httpStatuses, String(response.status()));
        let rateHint: number | null = null;
        if (response.status() === 429) {
          if (path.includes("/native/v1/") && response.request().method() === "GET") {
            readRateLimits++; resumesAtRateLimit = activeReads;
          }
          const header = await response.headerValue("retry-after");
          if (header === null) retryAfterMissing++;
          else {
            retryAfterPresent++;
            const seconds = boundedRetryAfter(header, Date.now());
            rateHint = seconds;
            if (seconds === null) retryAfterUnbounded++; else add(retryAfterSeconds, String(seconds));
          }
        }
        if (path === `/api/games/${gameId}` && response.request().method() === "GET") {
          add(recordHttpStatuses, String(response.status()));
          if (response.status() === 200) installRecord((await audit.json(response)).game);
          return;
        }
        if (/\/presentation\/[a-f0-9]{64}\.json$/.test(path) && response.status() === 200) {
          const artifact = await audit.json(response);
          check(artifact.catalog_sha256 === ready!.identity.catalog_sha256, "presentation_catalog");
          for (const card of ready!.cards) {
            const found = artifact.cards.find((row: { oracle_id: string }) => row.oracle_id === card.oracle_id);
            check(found?.faces.length === 1 && found.faces[0].name === card.name, "presentation_frozen_names");
          }
          artifactPath = path;
          presentationArtifacts++;
          presentationVerified = true;
        }
        if (/\/assets\/live-game-[^/]+\.js$/.test(path)) lazyLoaded = true;
        if (path === "/api/games/admission") {
          check(response.status() === 200, "admission_http");
          const value = await audit.json(response); admission = value.admission.state === "supported" && value.admission.launch_ready === true;
        } else if (path === "/api/games" && response.request().method() === "POST") {
          check(response.status() === 200, "launch_http");
          const value = await audit.json(response); check(value.game.state === "active", "launch_active");
          gameId = value.game.id; launched = true;
        } else if (/\/api\/games\/[^/]+\/seat$/.test(path)) {
          if (response.status() !== 200) return;
          const value = await audit.json(response), next = sdk.parseSeatBootstrap(value.bootstrap_file);
          check(next.viewer === 0 && next.match_id === gameId && next.server_origin === ready!.origin
            && next.engine_build_id === ready!.identity.engine_build_id && next.server_build_id === ready!.identity.server_build_id
            && next.catalog_sha256 === ready!.identity.catalog_sha256, "grant_identity");
          if (bootstrap) check(next.capability === bootstrap.capability, "regrant_credentials");
          bootstrap = next; grants++;
        } else if (path.includes("/native/v1/")) {
          const operation = path.endsWith("/events") ? "events" : path.endsWith("/resume") ? "resume"
            : path.endsWith("/presentation") ? "presentation" : path.endsWith("/manifest") ? "manifest"
              : path.endsWith("/actions/query") ? "query" : path.endsWith("/commands") ? "submit" : "execution";
          add(nativeStatuses, `${operation}_${response.status()}`);
          const queryOwner = queryOwners.get(response.request());
          if (operation === "query" && queryOwner && queryOwner === discovery) queryOwner.lastHttp = response.status();
          if (operation === "query" && response.status() === 429) {
            if (rateHint === null) submissionFailure ||= "query429_without_bounded_hint";
            else {
              queryRateLimits++; queryRateReadFloor = readOrdinal;
              if (queryOwner && queryOwner === discovery) { queryOwner.calls++; queryOwner.rateHint = true; }
            }
            return;
          }
          if (operation === "submit" && response.status() === 429) {
            const id = sdk.parsePreparedCommand(response.request().postData()!).envelope.command_id;
            if (rateHint === null) submissionFailure ||= "submission429_without_bounded_hint";
            else submissions.rateLimited(id, activeReads);
            return; // A transport/rate response never proves disposition or schema failure.
          }
          if (path.endsWith("/presentation") && response.status() === 404) { presentationMissing++; return; }
          // Reads may exercise the real controller's bounded retry path. Only a
          // later successful command/consumption qualifies recovered operation.
          if (response.request().method() === "GET" && [429, 503].includes(response.status())) return;
          if (operation === "execution" && response.status() === 404) {
            submissions.missing(decodeURIComponent(path.split("/").at(-1)!)); return;
          }
          check(response.status() === 200, "native_browser_http");
          const value = await audit.json(response);
          if (path.endsWith("/resume")) {
            check(value.viewer === 0 && value.match_id === gameId && value.engine_build_id === ready!.identity.engine_build_id
              && value.server_build_id === ready!.identity.server_build_id && value.catalog_sha256 === ready!.identity.catalog_sha256, "resume_identity");
            activeReads++; revision = value.next; outcome = value.outcome; lifecycle = value.lifecycle?.status ?? "";
            if (value.state.status === "available") observation = value.state.observation;
          } else if (path.endsWith("/events")) {
            revision = value.next;
            for (const entry of value.entries) {
              if (entry.publication.kind === "view_replaced") observation = entry.publication.observation;
              if (entry.publication.kind === "outcome") outcome = entry.publication.result;
              if (entry.publication.kind === "lifecycle") lifecycle = entry.publication.status.status;
            }
          } else if (path.endsWith("/actions/query")) {
            queries++;
            check(["complete", "incomplete", "stale", "unavailable", "cancelled"].includes(value.outcome.status), "query_outcome_kind");
            add(queryOutcomes, value.outcome.status);
            if (queryOwner && queryOwner === discovery) {
              queryOwner.calls++;
              if (value.outcome.status === "stale") queryOwner.nativeStale = true;
              if (["complete", "incomplete"].includes(value.outcome.status)) {
                check(value.outcome.page.token === queryOwner.context.token, "discovery_original_token");
                queryOwner.beginnings.push(...value.outcome.page.beginnings);
              }
            }
          } else if (path.endsWith("/commands")) {
            const id = sdk.parsePreparedCommand(response.request().postData()!).envelope.command_id;
            check(value.command_id === id && value.status === "accepted", "human_command_rejected");
            submissions.accepted(id); accepted.add(id);
          } else if (/\/commands\/[^/]+$/.test(path)) {
            const id = decodeURIComponent(path.split("/").at(-1)!);
            check(value.command_id === id, "execution_command_identity");
            if (value.status.status === "consumed_by_rules") { submissions.consumed(id); consumed.add(id); }
          }
          if (observation) check(observation.observation_version === OBSERVATION_VERSION && observation.view.viewer === 0, "actual_observation_21");
          const ordinal = readOrdinals.get(response.request());
          if (ordinal !== undefined) successfulReadOrdinal = Math.max(successfulReadOrdinal, ordinal);
        }
      });
      const id = submissionRequestIds.get(response.request());
      if (id) {
        submissionObservations.set(id, job);
        void job.finally(() => { if (submissionObservations.get(id) === job) submissionObservations.delete(id); });
      }
    });
    await context.route("**/*", route => audit.track(async () => {
      const request = route.request(), url = new URL(request.url()); requests++;
      try {
        check(!url.href.includes(ready!.owner.cookie) && !url.href.includes(ready!.foreign.cookie)
          && (!bootstrap || !url.href.includes(bootstrap.capability)), "secret_url");
        if (url.origin !== ready!.origin) { await route.abort(); return; }
        if (url.pathname.includes("/native/")) {
          nativeReads++;
          const headers = await request.allHeaders();
          check(headers.cookie?.includes(`better-auth.session_token=${ready!.owner.cookie}`), "native_account_cookie");
          if (/\/presentation\/[a-f0-9]{64}\.json$/.test(url.pathname)) check(request.method() === "GET" && !headers.authorization, "artifact_cookie_without_bearer");
          else if (!url.pathname.endsWith("/client-config.json")) check(bootstrap && headers.authorization === `Bearer ${bootstrap.capability}`, "native_human_bearer");
          if (request.method() === "POST") authenticatedPosts++;
          if (request.method() === "GET" && /\/(resume|events)$/.test(url.pathname)) readOrdinals.set(request, ++readOrdinal);
          if (url.pathname.endsWith("/actions/query") && discovery) queryOwners.set(request, discovery);
          if (/\/commands$/.test(url.pathname) && request.method() === "POST") {
            const body = request.postData(); check(body && !audit.protocolFailed && !submissionFailure, "unsolicited_human_post");
            const envelope = sdk.parsePreparedCommand(body).envelope;
            const previous = submissions.records.get(envelope.command_id);
            if (previous) {
              const observation = submissionObservations.get(envelope.command_id);
              if (observation) await bounded(observation, 5_000, "submission_response_observation");
              check(!audit.protocolFailed && !submissionFailure, "submission_response_fault");
            }
            const binding = { origin: ready!.origin, match: gameId, seat: "0", engineBuild: ready!.identity.engine_build_id,
              serverBuild: ready!.identity.server_build_id, catalog: ready!.identity.catalog_sha256, protocol: pins.protocol_schema_sha256 };
            const original: SavedIntent | undefined = previous?.original ?? (pending ? { ...pending, binding } : undefined);
            check(original && Object.keys(binding).length === Object.keys(original.binding).length
              && Object.entries(binding).every(([key, value]) => original.binding[key] === value), "original_journal_binding");
            check(envelope.expected_revision === original.revision && envelope.command.kind === "decision"
              && envelope.command.token === original.token, "original_command_context"); originalContexts++;
            check(envelope.command.kind === "decision", "decision_command");
            completeResponse(envelope.command.response, original.response); completeResponses++;
            check(await journal(page!, body, original.binding), "durable_exact_bytes"); journalBeforeSend++;
            const attempt = submissions.attempt(envelope.command_id, body, previous ? undefined : original);
            submissionRequestIds.set(request, envelope.command_id);
            add(responseAttemptKinds, envelope.command.response.response);
            if (!attempt.replay) { add(responseKinds, envelope.command.response.response); sent.set(envelope.command_id, body); }
          }
        }
        try { await route.continue(); }
        catch (error) {
          if (audit.expectedAbort(request.failure()?.errorText)) audit.expectedCancellations++;
          else throw error;
        }
      } catch { audit.protocolFailed = true; await route.abort().catch(() => {}); }
    }));
    const current = (): Observation => { check(observation, "browser_observation"); return observation; };
    const sendChoice = async (action: () => Promise<unknown>, response: PlayerResponse, original: { revision: string; token: string }, end = naturalEnd) => {
      check(!pending && submissions.active === null && !audit.protocolFailed && !submissionFailure, "command_gate");
      add(requestKinds, current().decision!.request);
      pending = { ...original, response }; const before = sent.size;
      try {
        await action();
        await until(() => !audit.protocolFailed && sent.size === before + 1, "human_post_observed", Math.min(20_000, Math.max(1, end - Date.now())));
        return [...sent.keys()].at(-1)!;
      } finally { pending = undefined; }
    };
    const finishChoice = async (id: string) => {
      let retryUsed = false;
      await untilRead(async () => {
        if (accepted.has(id) && consumed.has(id)) return true;
        const saved = submissions.records.get(id)!;
        const retry = page!.getByRole("button", { name: "Check or retry saved choice", exact: true });
        if (!retryUsed && saved.rateLimited && activeReads > saved.resumeAtRate && !recordPaused && !recordCompleted && !outcome
          && !readCooldownPending(readRateLimits, resumesAtRateLimit, activeReads) && await connected()
          && await retry.count() === 1 && await retry.isEnabled()) {
          check(await journal(page!, saved.body, saved.original.binding), "retry_original_journal"); retryUsed = true;
          try { await retry.click({ timeout: 1_000 }); submissionRetryClicks++; }
          catch (error) { if (error instanceof Error && error.name === "TimeoutError") submissionRetryTimeouts++; else throw error; }
        }
        return false;
      }, "native_consumed_by_rules");
      await until(() => journal(page!), "journal_resolved");
      submissions.resolve(id);
      // Only observed consumption and exact journal resolution reach this UI wait.
      await untilRead(() => page!.getByRole("region", { name: "Choice status" }).filter({ hasText: "The table used your choice." }).isVisible(), "choice_status");
    };
    const settle = async (action: () => Promise<unknown>, response: PlayerResponse, original = { revision, token: current().decision!.token }) => {
      await finishChoice(await sendChoice(action, response, original));
    };
    const choose = async (predicate: (beginning: ActionBeginning) => boolean) => {
      beginnings = []; // No prior selection/menu crosses a UI discovery chain.
      const button = page!.getByRole("button", { name: "Discover actions", exact: true });
      const phases = {
        complete: "Discovery complete. Each submitted action is validated again by the host.",
        limited: "Discovery reached a local limit. Results are not a complete menu.",
        uncertain: "Discovery could not finish. Results cannot be submitted.",
        stale: "The decision changed. Discover actions again.", unavailable: "Discovery is unavailable for this decision.",
        cancelled: "Discovery stopped. No game command was cancelled.",
      };
      const healthy = async () => {
        check(!shutdown.signal.aborted, "check_aborted");
        check(!audit.protocolFailed && audit.pageErrors === 0, "rate_recovery_protocol_or_page_error");
        check(!pending && await journal(page!), "rate_recovery_unknown_journal");
        check(!recordPaused && !recordCompleted && !outcome, "rate_recovery_terminal_record");
      };
      let selectedCommand: string | undefined;
      const currentContext = async (original: { revision: string; token: string }) => await connected()
        && revision === original.revision && current().decision?.token === original.token
        && !readCooldownPending(readRateLimits, resumesAtRateLimit, activeReads) && !queryCooldownPending();
      const chosen = await discoverWithRateRecovery<{ revision: string; token: string }, ActionBeginning[]>({
        allowed: naturalAttempted, now: Date.now, end: Math.min(naturalEnd, Date.now() + 90_000), healthy,
        capture: async () => {
          await untilRead(async () => await connected() && await button.getAttribute("aria-disabled") === "false", "discover_enabled");
          return { revision, token: current().decision!.token };
        }, rateCount: () => readRateLimits,
        ...(naturalAttempted ? { normalStale: {
          counts: staleRediscovery,
          changed: (previous: { revision: string; token: string }, next: { revision: string; token: string }) => ({ token: previous.token !== next.token, cursor: previous.revision !== next.revision }),
          ready: async (end: number) => {
            // Native/optimistic stale is a UI request to discover afresh. Keep
            // the single product read owner; never poll the SDK here.
            beginnings = [];
            await until(async () => {
              await healthy();
              check(await page!.getByRole("heading", { name: /^(Rate limit or possible contention|Host temporarily unavailable|Connection lost|Seat authorization rejected|Incompatible table or presentation)$/ }).count() === 0, "stale_rediscovery_persistent_failure");
              return await connected() && !readCooldownPending(readRateLimits, resumesAtRateLimit, activeReads) && !queryCooldownPending()
                && await button.getAttribute("aria-disabled") === "false";
            }, "stale_rediscovery_deadline", Math.max(1, end - Date.now()));
          },
        } } : {}),
        usable: async original => {
          const usable = await connected() && revision === original.revision && current().decision?.token === original.token
            && !readCooldownPending(readRateLimits, resumesAtRateLimit, activeReads) && !queryCooldownPending();
          if (!usable) invalidatedCompleteMenus++;
          return usable;
        },
        recovered: async end => {
          await until(async () => {
            await healthy();
            check(await page!.getByRole("heading", { name: /^(Rate limit or possible contention|Host temporarily unavailable|Connection lost|Seat authorization rejected|Incompatible table or presentation)$/ }).count() === 0, "rate_recovery_persistent_failure");
            return activeReads > resumesAtRateLimit && !queryCooldownPending() && await connected()
              && await button.getAttribute("aria-disabled") === "false";
          }, "rate_recovery_deadline", Math.max(1, end - Date.now()));
          return readRateLimits;
        },
        use: async (original, values, end) => {
          if (!await currentContext(original)) { lateNoSendInvalidations++; return false; }
          const index = values.findIndex(predicate); check(index >= 0, "permitted_beginning");
          const beginning = values[index]!;
          check(beginning.initiation.action === "pass" || beginning.label !== "unavailable", "producer_action_label");
          if (beginning.initiation.action === "finish_mana" && beginning.initiation.undo_mana) {
            check(beginning.label !== "unavailable" && beginning.label.available.text === "Finish adding mana; reverse reversible mana if payment fails", "conditional_reversal_wording");
            conditionalReversal = true;
          }
          const menu = page!.getByRole("region", { name: "Available actions", exact: true });
          const action = menu.locator("ol > li > button").nth(index);
          check(await menu.locator("ol > li > button").count() === values.length, "discovery_current_menu");
          const before = sent.size;
          try {
            // A short actionability deadline avoids waiting on an old disabled
            // draft across reconnect. No forced click or SDK command is used.
            selectedCommand = await sendChoice(() => action.click({ timeout: Math.min(1_000, Math.max(1, end - Date.now())) }), initiationResponse(beginning.initiation), original, end);
            return true;
          } catch (error) {
            const timeout = error instanceof Error && error.name === "TimeoutError";
            if (timeout) actionClickTimeouts++;
            if (!(timeout || error instanceof Error && error.message === "human_post_observed") || sent.size !== before) throw error;
            await healthy(); // Includes empty journal; attempted/unknown commands never retry.
            if (await currentContext(original) && await action.getAttribute("aria-disabled") !== "true") throw error;
            lateNoSendInvalidations++; return false;
          }
        },
        discover: async (original, end) => {
          // The returned command context belongs to this fresh discovery,
          // never to the discarded stale menu.
          const attempt: Discovery = { context: original, beginnings: [], calls: 0, nativeStale: false, lastHttp: 0, rateHint: false }; discovery = attempt;
          await page!.getByLabel("Discovery scope", { exact: true }).selectOption("");
          await button.click(); discoveryGenerations++; let phase = "";
          try {
            await until(async () => {
              if (!attempt.calls) return false;
              for (const [name, text] of Object.entries(phases)) if (await page!.getByText(text, { exact: true }).isVisible()) { phase = name; return true; }
              return false;
            }, "discovery_complete", Math.min(20_000, Math.max(1, end - Date.now())));
          } finally {
            if (discovery === attempt) discovery = null;
            lastDiscoveryCalls = attempt.calls; lastDiscoveryBeginnings = attempt.beginnings.length;
            lastDiscoveryPassOffered = attempt.beginnings.some(value => value.initiation.action === "pass");
            add(discoveryPhases, phase || "deadline");
          }
          const rateInvalidated = queryRateInvalidated(phase, attempt.lastHttp, attempt.rateHint);
          if (rateInvalidated) queryRateInvalidations++;
          return { phase: rateInvalidated ? "stale" : phase, value: attempt.beginnings, nativeStale: attempt.nativeStale };
        },
      }, rateRecovery);
      beginnings = chosen.value;
      check(selectedCommand, "human_post_observed");
      await finishChoice(selectedCommand);
      if (chosen.recovered) recoveredCommands++;
      if (chosen.rediscovered) staleRecoveredCommands++;
    };
    await page.goto(`${ready.origin}/play?deck=${ready.deckId}`);
    // Use the actual custom product selector, not URL-derived deck authority.
    if (!(await page.getByRole("heading", { name: "Native foundations default", exact: true }).count())) {
      await page.getByRole("combobox", { name: "Your deck" }).click();
      await page.getByRole("option", { name: "Native foundations default", exact: true }).click();
    }
    await page.getByRole("button", { name: "Check compatibility", exact: true }).click();
    await until(() => admission && !audit.protocolFailed, "supported_admission");
    check(!lazyLoaded, "session_not_loaded_during_setup");
    await page.getByRole("button", { name: "Start game", exact: true }).click();
    await until(() => launched && gameId && new URL(page!.url()).pathname === `/play/games/${gameId}`, "active_route", 170_000);
    await until(connected, "lazy_session_connected");
    check(lazyLoaded, "lazy_session_chunk");
    await until(() => Boolean(observation), "opening_observation");
    check(grants === 1, "single_initial_grant");
    const commander = current().view.objects.find(object => object.owner === 0 && object.zone.zone === "command");
    check(commander, "native_default_commander"); const commanderOracle = commander.oracle_id;
    await until(() => presentationVerified, "production_presentation_required");
    const commanderName = ready.cards.find(card => card.oracle_id === commanderOracle)?.name;
    check(commanderName, "frozen_commander_name");
    check(await page.getByRole("region", { name: "Command zone", exact: true }).getByRole("button", { name: `Inspect ${commanderName}`, exact: true }).count() === 1, "named_commander_ui"); namedCards++;
    const expectedHandNames = new Map<string, number>();
    for (const object of current().view.objects.filter(object => object.owner === 0 && object.zone.zone === "hand")) {
      const name = ready.cards.find(card => card.oracle_id === object.oracle_id)?.name;
      check(name, "frozen_hand_name"); expectedHandNames.set(name, (expectedHandNames.get(name) ?? 0) + 1);
    }
    for (const [name, count] of expectedHandNames) {
      check(await page.getByRole("region", { name: /^Your hand/ }).getByRole("button", { name: `Inspect ${name}`, exact: true }).count() === count, "named_hand_ui");
      namedCards += count;
    }
    if (pauseCase) {
      check(current().decision?.request === "mulligan" && sent.size === 0, "pause_opening_withheld");
      await until(() => recordState === "interrupted" && recordPauseCode === "human_timeout", "actual_human_timeout", 35_000);
      await inspectPauseUI();
      await until(async () => {
        pauseBannerVisible = await page!.getByRole("heading", { name: "Game paused", exact: true }).isVisible();
        interruptionBannerVisible = await page!.getByRole("heading", { name: "Game interrupted", exact: true }).isVisible();
        return pauseBannerVisible || interruptionBannerVisible;
      }, "timeout_banner");
      pauseCauseVisible = await page.getByRole("status").filter({ hasText: "The game stopped after the human decision time limit. Its saved record is retained." }).isVisible();
      commandControlsDisabled = await page.getByRole("button", { name: "Confirm choice", exact: true }).isDisabled()
        && await page.getByRole("radio", { name: "Keep hand", exact: true }).isDisabled();
      check(commandControlsDisabled && sent.size === 0 && recordOutcome === null, "pause_controls_and_no_commands");
      check(await page.getByRole("heading", { name: "Game complete", exact: true }).count() === 0
        && await page.getByText(/^Rules outcome:/).count() === 0
        && await page.getByText("You won.", { exact: true }).count() === 0
        && await page.getByText("The game ended in a draw.", { exact: true }).count() === 0, "pause_is_not_rules_outcome");
      await page.getByRole("region", { name: "Command zone", exact: true }).getByRole("button", { name: `Inspect ${commanderName}`, exact: true }).click();
      check(await page.getByRole("complementary", { name: "Card inspector", exact: true }).isVisible(), "paused_inspection_available");
      check(await journal(page), "pause_journal_empty");
      check(interruptionBannerVisible && !pauseBannerVisible, "timeout_banner_contract");
      check(pauseCauseVisible, "pause_cause_visible"); resourcePauseVerified = true;
    } else {
    const openingContext = { revision, token: current().decision!.token };
    await page.getByRole("radio", { name: "Keep hand", exact: true }).check();
    await settle(() => page!.getByRole("button", { name: "Confirm choice", exact: true }).click(), { response: "mulligan", take: false }, openingContext);
    for (let i = 0; i < 12; i++) {
      await until(() => current().decision?.request === "priority", "priority_window");
      if (current().view.turn.step === "precombat_main") break;
      await choose(value => value.initiation.action === "pass");
    }
    check(current().view.turn.step === "precombat_main" && current().view.turn.number === 1, "first_main");
    await choose(value => value.initiation.action === "play_land");
    landPlayed = current().view.turn.lands_played === 1;
    check(landPlayed, "land_played");
    await choose(value => value.initiation.action === "cast" && value.initiation.card === commander.handle
      && value.label !== "unavailable" && value.label.available.source?.kind === "spell"
      && value.label.available.source.oracle_id === commanderOracle && value.label.available.source.object === commander.handle);
    await until(() => current().decision?.request === "mana", "mana_window");
    await choose(value => value.initiation.action === "activate_mana" && value.label !== "unavailable"
      && value.label.available.source?.kind === "mana_ability" && value.label.available.source.object === value.initiation.object
      && Boolean(value.label.available.source.text));
    await choose(value => value.initiation.action === "finish_mana" && value.initiation.undo_mana);
    await until(() => current().decision?.request === "spend_mana", "payment_window");
    const payment = current().decision; check(payment?.request === "spend_mana" && payment.cost.generic === "0" && current().decision_context?.source, "permitted_payment");
    const paymentContext = { revision, token: payment.token };
    const colors = { W: "White", U: "Blue", B: "Black", R: "Red", G: "Green", C: "Colorless" };
    const spending: Record<string, string> = {};
    for (const color of Object.keys(payment.available) as (keyof typeof colors)[]) {
      const amount = payment.cost.colored.filter(required => required === color).length.toString();
      if (amount !== "0") spending[color] = amount;
      await page.getByRole("textbox", { name: new RegExp(`^${colors[color]}`) }).fill(amount);
    }
    await settle(() => page!.getByRole("button", { name: "Confirm choice", exact: true }).click(), { response: "spend_mana", spending }, paymentContext);
    const resolved = () => current().view.objects.some(object => object.owner === 0 && object.oracle_id === commanderOracle && object.zone.zone === "battlefield");
    for (let i = 0; i < 8 && !resolved(); i++) {
      await until(() => current().decision?.request === "priority" || resolved(), "resolution_priority");
      if (!resolved()) await choose(value => value.initiation.action === "pass");
    }
    await until(resolved, "commander_resolved"); commanderResolved = true;
    const readsBefore = activeReads, grantBefore = grants;
    observation = null; await page.reload();
    await until(() => grants === grantBefore + 1 && activeReads > readsBefore && observation && resolved(), "fresh_grant_recovery");
    await until(connected, "recovered_connection"); check(await journal(page), "recovery_journal_empty"); recovered = true;
    await choose(value => value.initiation.action === "pass");
    // Unmount the lazy session, then let any already in-flight read settle. No
    // later native I/O or UI command may retain the discarded route's File.
    await page.getByRole("link", { name: "Back to saved games", exact: true }).click();
    await audit.drain(5_000); await delay(250);
    const readsAfterUnmount = nativeReads; await delay(300);
    check(nativeReads === readsAfterUnmount && !(await connected()), "old_file_route_unmounted"); unmounted = true;
    const priorGrant = grants;
    await page.goto(`${ready.origin}/play/games/${gameId}`);
    await until(() => grants === priorGrant + 1, "remount_fresh_file"); await until(connected, "remount_connection"); regranted = true;
    await choose(value => value.initiation.action === "pass");
    // Real cookie/tenant negatives use separate browser contexts; their requests
    // never touch the owning human session or its read controller.
    for (const foreign of [true, false]) {
      const other = await browser.newContext({ serviceWorkers: "block" });
      try {
        if (foreign) await other.addCookies([cookie(ready.foreign.cookie)]);
        const denied = await other.request.post(`${ready.origin}/api/games/${gameId}/seat`, { data: {}, headers: { Origin: ready.origin }, timeout: 5_000 });
        check(denied.status() === (foreign ? 404 : 401), "foreign_seat_denied");
        const bytes = await denied.text(); check(!bytes.includes(bootstrap!.capability) && !bytes.includes("bootstrap_file"), "no_foreign_human_file"); negatives++;
        const access = await other.request.get(`${ready.origin}/api/games/${gameId}/native/v1/matches/${gameId}/resume`, { headers: { Authorization: `Bearer ${bootstrap!.capability}` }, timeout: 5_000 });
        check(access.status() === (foreign ? 404 : 401), "cookie_and_bearer_required"); negatives++;
        const artifact = await other.request.get(`${ready.origin}${artifactPath}`, { timeout: 5_000 });
        check(artifact.status() === (foreign ? 404 : 401), "artifact_actor_required"); negatives++;
        const otherPage = await other.newPage(); await otherPage.goto(`${ready.origin}/play/games/${gameId}`);
        await until(() => foreign ? otherPage.getByText(/This game (session is unavailable|is no longer available from your account)/).isVisible() : otherPage.getByRole("main", { name: "Sign in", exact: true }).isVisible(), "foreign_route_denied");
        check(await otherPage.getByRole("heading", { name: "Connected · Seat 1", exact: true }).count() === 0, "no_foreign_session"); negatives++;
      } finally { await other.close(); }
    }
    const noBearer = await context.request.get(`${ready.origin}/api/games/${gameId}/native/v1/matches/${gameId}/resume`, { timeout: 5_000 });
    check(noBearer.status() === 401, "owner_cookie_without_bearer_denied"); negatives++;
    // Each fresh File revalidates immutable assets. Command-only refreshes keep
    // those verified facts while still obtaining their own coherent resumes.
    const failedReads = Object.entries(nativeStatuses).some(([key, count]) => count > 0 && /_(429|503)$/.test(key));
    if (!failedReads) {
      immutableAssetsRevalidated = nativeStatuses.manifest_200 === grants && nativeStatuses.presentation_200 === grants
        && presentationArtifacts === grants && activeReads >= sent.size + grants;
      check(immutableAssetsRevalidated, "immutable_assets_and_fresh_resumes");
    }
    // Continue only through explicit DOM choices from permitted requests. This
    // uses only the resolved commander, offered defenders and pass/draw choices.
    // There is no imported human policy or product-side automatic action.
    if (natural) {
      naturalAttempted = true; const end = Date.now() + naturalLimitMs; naturalEnd = end;
      while (Date.now() < end && sent.size < 2_000 && !audit.protocolFailed) {
        check(!shutdown.signal.aborted, "check_aborted");
        // Observe the product route's own serial Actor-record polling. No test
        // polling consumes its shared 240/min UI budget.
        if (recordCompleted) { naturalCompleted = true; break; }
        if (recordPaused) { naturalPaused = lifecycle === "resource_paused"; break; }
        if (outcome) { naturalCompleted = true; break; }
        if (["resource_paused", "faulted", "cancelled", "rejected"].includes(lifecycle)) { naturalPaused = lifecycle === "resource_paused"; break; }
        const request = current().decision;
        if (!request) { await delay(50); continue; }
        const original = { revision, token: request.token };
        if (request.request === "priority") await choose(value => value.initiation.action === "pass");
        else if (request.request === "attackers" || request.request === "blockers") {
          const title = request.request === "attackers" ? "Declare attackers" : "Declare blockers";
          const form = page.getByRole("group", { name: title, exact: true });
          const controls = form.getByRole("combobox");
          check(await controls.count() === request.candidates.length, "permitted_combat_candidates");
          const attacks: [string, number][] = [];
          for (const [i, handle] of request.candidates.entries()) {
            const isCommander = request.request === "attackers" && current().view.objects.some(object => object.handle === handle
              && object.owner === 0 && object.oracle_id === commanderOracle);
            const defender = request.request === "attackers" ? request.defenders[0] : undefined;
            if (isCommander && defender !== undefined) {
              await controls.nth(i).selectOption(JSON.stringify([defender])); attacks.push([handle, defender]);
            } else await controls.nth(i).selectOption("unused");
          }
          await settle(() => page!.getByRole("button", { name: "Confirm choice", exact: true }).click(), request.request === "attackers" ? { response: "attackers", attacks } : { response: "blockers", blocks: [] }, original);
        } else if (request.request === "damage" && request.recipients.length === 0 && request.player_recipient !== null && request.player_recipient !== undefined) {
          await page.getByRole("textbox", { name: `Seat ${request.player_recipient + 1}`, exact: false }).fill(request.power);
          await settle(() => page!.getByRole("button", { name: "Confirm choice", exact: true }).click(), { response: "trample_damage", assignments: [], player: request.player_recipient, amount: request.power }, original);
        } else if (request.request === "discard") {
          const boxes = page.getByRole("group", { name: "Choose cards to discard", exact: true }).getByRole("checkbox");
          check(await boxes.count() === request.candidates.length, "permitted_discard_candidates");
          for (let i = 0; i < request.count; i++) await boxes.nth(i).check();
          await settle(() => page!.getByRole("button", { name: "Confirm choice", exact: true }).click(), { response: "discard", objects: request.candidates.slice(0, request.count) }, original);
        } else { add(requestKinds, request.request); naturalUnsupported = true; break; }
      }
      naturalCompleted ||= Boolean(outcome); naturalBounded = !naturalCompleted && !naturalPaused && !naturalUnsupported && !recordPaused && !["faulted", "cancelled", "rejected"].includes(lifecycle);
    }
    }
    check(await page.evaluate(() => localStorage.length === 0 && sessionStorage.length === 0 && location.search === "" && location.hash === ""), "no_secret_storage_or_url");
    check(!audit.protocolFailed && !submissionFailure && audit.pageErrors === 0 && submissions.attempts === journalBeforeSend && submissions.attempts === originalContexts
      && submissions.attempts === completeResponses && sent.size === accepted.size && sent.size === consumed.size
      && submissions.resolutions === sent.size, "all_command_proofs");
    if (!pauseCase) check(["mulligan", "pass", "play_land", "cast", "activate_mana", "finish_mana", "spend_mana"].every(kind => responseKinds[kind]), "first_turn_responses");
  } catch (error) {
    // Only our short fixed assertion codes may become evidence; Playwright and
    // SDK exceptions can include authenticated DOM, headers or private bodies.
    failure = error instanceof Error && /^[a-z][a-z0-9_]{0,80}$/.test(error.message) ? error.message : "browser_operation_failed";
  } finally {
    if (naturalAttempted && context && page && ready && gameId) {
      try {
        if (outcome && !recordCompleted && !recordPaused) await until(() => recordCompleted || recordPaused, "terminal_record_observed", 20_000);
        const recorded = recordOutcome as RecordedOutcome | null;
        if (recordCompleted && recorded) {
          naturalCompleted = true; naturalBounded = false;
          const expected = recorded.kind === "draw" ? ["Draw", "The game ended in a draw."]
            : [`Seat ${recorded.winner + 1} wins`, recorded.winner === 0 ? "You won." : `Automated seat ${recorded.winner + 1} won.`];
          const end = Date.now() + 20_000;
          while (Date.now() < end && !uiOutcomeMatches) {
            for (const text of expected) {
              const matches = page.getByText(text, { exact: true });
              for (let i = 0; i < await matches.count(); i++) if (await matches.nth(i).isVisible()) uiOutcomeMatches = true;
            }
            if (!uiOutcomeMatches) await delay(100);
          }
          if (outcome) check((outcome as RulesResult).outcome === recorded.kind
            && (recorded.kind !== "win" || (outcome as Extract<RulesResult, { outcome: "win" }>).player === recorded.winner), "native_record_outcome_match");
          if (!uiOutcomeMatches) failure ||= "terminal_outcome_not_visible";
        }
        if (recordPaused) {
          const inactive = await inspectPauseUI();
          naturalPauseUiObserved = inactive.commandsBlocked;
          naturalPauseCauseVisible = inactive.causeVisible;
          naturalPauseControls = inactive.controls;
        }
        journalUnresolved = !(await journal(page));
      } catch { failure ||= "terminal_comparison_failed"; }
    }
    // Final acceptance follows closure and observer drain. Only evidenced body
    // transport cancellation is expected; late schema/context/page errors fail.
    audit.closing = true;
    // Inspect terminal/paused controls before releasing the File clears them.
    // Failure or a stalled release still proceeds to independent host cleanup.
    try {
      const release = page?.getByRole("button", { name: "Release seat and clear view", exact: true });
      if (release && await release.count()) await release.click({ timeout: 5_000 });
    } catch { failure ||= "seat_release_failed"; }
    try {
      await bounded(Promise.allSettled([context?.close(), browser?.close()]), 5_000, "browser_cleanup");
      check(!browser?.isConnected(), "browser_cleanup");
      await audit.drain(5_000);
    } catch { failure ||= "browser_cleanup"; }
    const command = (file: string, args: string[], signal: AbortSignal, timeout: number) => new Promise<void>((done, fail) => {
      execFile(file, args, { signal, timeout, killSignal: "SIGKILL" }, error => error ? fail(Error("owned_join")) : done());
    });
    const ending = await finishOwnedChild(child, async signal => {
      let leases = 0;
      const root = join(directory, "private");
      for (const entry of existsSync(root) ? readdirSync(root, { withFileTypes: true }) : []) {
        if (!entry.isDirectory() || !/^[a-f0-9]{32}$/.test(entry.name)) continue;
        const owned = join(root, entry.name);
        await command(join(inputs.nativeRoot, "mtg-server"), ["lifecycle-join", owned], signal, 15_000);
        for (const seat of [1, 2, 3]) if (existsSync(join(owned, `bot-${seat}`, "lease"))) {
          await command("flock", ["--nonblock", join(owned, `bot-${seat}`, "lease"), "true"], signal, 2_000); leases++;
        }
      }
      return leases;
    });
    const cleanup = stopped && ending.complete && ending.leases === (launched ? 3 : 0);
    joinedLeases = ending.leases;
    if (!cleanup) failure ||= "owned_leases_not_joined";
    try { await audit.drain(5_000); } catch { failure ||= "observer_drain_deadline"; }
    const final = audit.verdict(failure || submissionFailure, submissions.attempts === journalBeforeSend && submissions.attempts === originalContexts
      && submissions.attempts === completeResponses && sent.size === accepted.size && sent.size === consumed.size && submissions.resolutions === sent.size,
      natural ? { recordState, recordedOutcome: recordOutcome as RecordedOutcome | null, observedOutcome: outcome as RulesResult | null,
        outcomeVisible: uiOutcomeMatches, journalUnresolved } : undefined);
    failure = final.failure;
    const receipt = { browser: name, version: browser?.version() ?? "unavailable", source: pins, checker_sources: checkerSources, product_manifest_sha256: fileHash(join(dist, ".vite/manifest.json")), observation_version: OBSERVATION_VERSION,
      passed: final.passed, failure, page_errors: audit.pageErrors, protocol_failed: audit.protocolFailed, lazy_loaded: lazyLoaded, admission, launched,
      grants, recovered, unmounted, regranted, negative_checks: negatives, requests, native_requests: nativeReads, authenticated_posts: authenticatedPosts,
      human_commands: sent.size, journal_before_send: journalBeforeSend, original_contexts: originalContexts, complete_responses: completeResponses,
      submission_attempts: submissions.attempts, submission_replays: submissions.replays, submission429: submissions.rateLimits,
      submission_execution404: submissions.missingExecutions, submission_resolutions: submissions.resolutions, submission_recovered_commands: submissions.recovered,
      submission_retry_ui_clicks: submissionRetryClicks, submission_retry_ui_timeouts: submissionRetryTimeouts, submission_transport_failure: submissionFailure,
      submission_attempt_bound_per_command: 5, submission_retry_ui_click_bound_per_command: 1,
      query_transport429: queryRateLimits, query_rate_invalidations: queryRateInvalidations, query_rate_read_pending: queryCooldownPending(),
      response_attempt_counts: responseAttemptKinds,
      accepted: accepted.size, consumed: consumed.size, action_queries: queries, request_counts: requestKinds, response_counts: responseKinds,
      native_status_counts: nativeStatuses, http_status_counts: httpStatuses, record_http_status_counts: recordHttpStatuses,
      retry_after_present: retryAfterPresent, retry_after_missing: retryAfterMissing, retry_after_unbounded: retryAfterUnbounded, retry_after_seconds_counts: retryAfterSeconds,
      read_rate_limits: readRateLimits, rate_recoveries_started: rateRecovery.started, rate_recoveries_ready: rateRecovery.ready,
      rate_recovery_handled_limits: rateRecovery.handled, rate_evidence_retired_limits: rateRecovery.retired,
      read_cooldown_pending: readCooldownPending(readRateLimits, resumesAtRateLimit, activeReads),
      rate_recovery_fresh_discoveries: rateRecovery.freshDiscoveries, rate_recovery_consumed_commands: recoveredCommands,
      read_cooldown_waits: readCooldownWaits, read_cooldown_ready: readCooldownReady,
      discovery_completed_menus_invalidated: invalidatedCompleteMenus, late_no_send_invalidations: lateNoSendInvalidations, action_click_timeouts: actionClickTimeouts,
      discovery_ui_generations: discoveryGenerations, stale_discovery_counts: staleRediscovery,
      stale_rediscovery_consumed_commands: staleRecoveredCommands,
      fixture_stress_raised_limits: stressRaisedLimits, default_ui_budget: !stressRaisedLimits && ready?.uiRateBudget === 240 && ready?.uiRateWindowMs === 60_000,
      ui_rate_budget: ready?.uiRateBudget ?? 0, ui_rate_window_ms: ready?.uiRateWindowMs ?? 0, query_outcome_counts: queryOutcomes, discovery_phase_counts: discoveryPhases,
      last_discovery_calls: lastDiscoveryCalls, last_discovery_beginnings: lastDiscoveryBeginnings, last_discovery_pass_offered: lastDiscoveryPassOffered,
      presentation_404: presentationMissing, presentation_artifacts: presentationArtifacts, production_presentation_verified: presentationVerified,
      immutable_assets_revalidated_per_grant: immutableAssetsRevalidated, named_cards_verified: namedCards,
      land_played: landPlayed, commander_resolved: commanderResolved, conditional_reversal_wording: conditionalReversal,
      natural_attempted: naturalAttempted, natural_completed: naturalCompleted, natural_resource_paused: naturalPaused, natural_bounded: naturalBounded,
      natural_game_verified: natural && final.passed, natural_pause_ui_observed: naturalPauseUiObserved,
      natural_pause_cause_visible: naturalPauseCauseVisible, natural_pause_command_controls: naturalPauseControls,
      natural_bound_ms: naturalLimitMs, natural_human_command_bound: natural ? 2_000 : 0, discovery_ui_click_bound_per_choice: 3,
      natural_unsupported: naturalUnsupported, rules_win: (outcome as RulesResult | null)?.outcome === "win", rules_draw: (outcome as RulesResult | null)?.outcome === "draw", cleanup_complete: cleanup, joined_bot_leases: joinedLeases, observer_shutdown_cancellations: audit.expectedCancellations,
      owned_child_exited: ending.exited, owned_child_escalated: ending.escalated, owned_ipc_disconnected: ending.ipcDisconnected };
    Object.assign(receipt, { game_record_reads: recordReads, game_record_completed: recordCompleted, game_record_paused: recordState === "paused", game_record_interrupted: recordState === "interrupted",
      game_record_win: (recordOutcome as RecordedOutcome | null)?.kind === "win", game_record_draw: (recordOutcome as RecordedOutcome | null)?.kind === "draw", visible_outcome_matches: uiOutcomeMatches,
      native_outcome_observed: Boolean(outcome), journal_unresolved: journalUnresolved, game_record_pause_counts: recordPauseCounts,
      coordinator_timeout_case: pauseCase, coordinator_timeout_verified: resourcePauseVerified, command_controls_disabled: commandControlsDisabled,
      game_record_state_counts: recordStateCounts, timeout_pause_banner_visible: pauseBannerVisible,
      timeout_interruption_banner_visible: interruptionBannerVisible, timeout_cause_visible: pauseCauseVisible });
    writeFileSync(join(directory, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n", { mode: 0o600, flag: "wx" });
    // Only fixed booleans/counts/public source identities, never raw diagnostics.
    console.log(JSON.stringify(receipt));
  }
  return !failure;
}

async function main() {
  if (process.argv.includes("--help")) {
    console.log("node check-product-session.ts --native-root DIR --sdk-root DIR --qualification JSON --records ARCHIVE_JSON_OR_JSONL --records-sha256 SHA256 --out NEW_DIR [--browser chromium|firefox|both] [--natural] [--natural-ms 240000] [--pause] [--stress-raised-limits]"); return;
  }
  process.umask(0o077);
  check(!(process.argv.includes("--pause") && process.argv.includes("--natural")), "exclusive_fixture_cases");
  const inputs: Inputs = { nativeRoot: required("native-root"), sdkRoot: required("sdk-root"), qualification: required("qualification"), records: required("records"), recordsHash: option("records-sha256") ?? "" };
  verifyInputs(inputs); // Before building, fixture startup or opening a browser.
  const output = required("out"); mkdirSync(output, { mode: 0o700 });
  const dist = join(output, "build");
  const log = openSync(join(output, "build.log"), "wx", 0o600);
  try {
    const build = spawnSync(join(repository, "packages/multiverse/node_modules/.bin/vite"), ["build", "--outDir", dist, "--emptyOutDir", "--logLevel", "warn"], { cwd: join(repository, "packages/multiverse"), stdio: ["ignore", log, log] });
    check(build.status === 0, "production_build");
  } finally { closeSync(log); }
  const selected = option("browser") ?? "both"; check(["chromium", "firefox", "both"].includes(selected), "browser_argument");
  let passed = true;
  for (const name of selected === "both" ? ["chromium", "firefox"] as const : [selected as "chromium" | "firefox"])
    passed = await run(name, output, dist, inputs, process.argv.includes("--natural") && name === "chromium", process.argv.includes("--pause")) && passed;
  check(passed, "product_acceptance_failed");
}
void main().catch(error => { console.error(error instanceof Error && /^[a-z][a-z0-9_]{0,80}$/.test(error.message) ? error.message : "product_check_failed"); process.exitCode = 1; });
