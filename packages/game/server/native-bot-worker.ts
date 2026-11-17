/** Separate process, one capability and one durable exact-command journal.
 * Stdin EOF aborts all network work; the parent joins this process before stop. */
import { existsSync } from "node:fs";
import { join } from "node:path";
import { randomBytes } from "node:crypto";
import { setTimeout as delay } from "node:timers/promises";
import { GameClient, parseSeatBootstrap, parsePreparedCommand, prepareCommand, HttpError, PROTOCOL_VERSION, type PlayerResponse } from "@multiverse/game-server-sdk";
import { foundationsPolicy } from "@multiverse/game-server-sdk/policies";
import { readPrivate, writePrivate } from "./native-private.ts";

const [directory, file, profile, logicalOrigin] = process.argv.slice(2);
const stop = new AbortController();
process.stdin.on("end", () => stop.abort()); process.stdin.on("data", () => stop.abort()); process.stdin.resume();
process.once("SIGTERM", () => stop.abort()); process.once("SIGINT", () => stop.abort());
try {
  if (!directory || !file || !logicalOrigin || profile !== "commander-foundations-v1") throw new Error("Unsupported bot profile");
  const bootstrap = parseSeatBootstrap(readPrivate(file, 4096));
  if (![1, 2, 3].includes(bootstrap.viewer)) throw new Error("Invalid automated seat");
  const client = new GameClient({ baseURL: bootstrap.server_origin, matchId: bootstrap.match_id, capability: bootstrap.capability });
  const request = <T>(operation: (signal: AbortSignal) => Promise<T>) => operation(AbortSignal.any([stop.signal, AbortSignal.timeout(30_000)]));
  const manifest = await request(signal => client.manifest({ signal }));
  if (manifest.profile !== profile || manifest.engine_build_id !== bootstrap.engine_build_id || manifest.server_build_id !== bootstrap.server_build_id || manifest.catalog_sha256 !== bootstrap.catalog_sha256) throw new Error("Bot identity differs");
  const binding = JSON.stringify([logicalOrigin, bootstrap.match_id, bootstrap.viewer, bootstrap.engine_build_id, bootstrap.server_build_id, bootstrap.catalog_sha256]);
  const journalPath = join(directory, "journal.json");
  let journal: { binding: string; exactBody: string } | null = existsSync(journalPath) ? JSON.parse(readPrivate(journalPath)) : null;
  if (journal && journal.binding !== binding) throw new Error("Bot journal identity differs");
  if (journal) parsePreparedCommand(journal.exactBody);
  const save = () => writePrivate(journalPath, JSON.stringify(journal));
  let ready = false;
  while (!stop.signal.aborted) {
    if (journal) {
      const original = parsePreparedCommand(journal.exactBody);
      let missing = false;
      try {
        const execution = await request(signal => client.execution(original.envelope.command_id, { signal }));
        if (execution.status.status !== "pending") {
          const failed = execution.status.status === "rejected" || execution.status.status === "cancelled";
          journal = null; save();
          if (failed) throw new Error("Automated command rejected");
          continue;
        }
      } catch (error) { if (error instanceof HttpError && error.status === 404) missing = true; else throw error; }
      if (missing) {
        // Exact original body/ID/token/revision, including after process death or
        // a lost acceptance response. No fresh decision can replace ambiguity.
        const accepted = await request(signal => client.submitExact(journal!.exactBody, { signal }));
        if (accepted.status === "rejected") {
          journal = null; save();
          throw new Error("Automated command rejected");
        }
      }
      await delay(100, undefined, { signal: stop.signal }); continue;
    }
    const resume = await request(signal => client.resume({ signal }));
    if (resume.viewer !== bootstrap.viewer || resume.engine_build_id !== bootstrap.engine_build_id || resume.server_build_id !== bootstrap.server_build_id || resume.catalog_sha256 !== bootstrap.catalog_sha256) throw new Error("Bot resume differs");
    if (!ready) { process.stdout.write('{"ready":true}\n'); ready = true; }
    if (resume.outcome) break;
    if (resume.lifecycle && ["faulted", "resource_paused", "cancelled", "rejected"].includes(resume.lifecycle.status)) throw new Error("Native execution paused");
    if (resume.state.status === "available" && resume.state.observation.decision) {
      const observation = resume.state.observation;
      const response: PlayerResponse | null = foundationsPolicy(observation, bootstrap.viewer);
      if (!response) throw new Error("Unsupported automated decision");
      const command = prepareCommand({ protocol_version: PROTOCOL_VERSION, command_id: randomBytes(16).toString("hex"), expected_revision: resume.next, command: { kind: "decision", token: observation.decision!.token, response } });
      journal = { binding, exactBody: command.exactBody }; save();
    } else await delay(100, undefined, { signal: stop.signal });
  }
} catch {
  // Never print SDK errors, responses, observations, paths or capabilities.
  if (!stop.signal.aborted) process.exitCode = 1;
} finally { process.stdin.destroy(); }
