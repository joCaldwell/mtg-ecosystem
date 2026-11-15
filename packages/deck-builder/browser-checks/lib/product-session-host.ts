// Private IPC only: signed cookies never enter argv, URLs, files or logs.
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { startHostedFixture, privateTree, type Inputs } from "./product-session-fixture.ts";

let fixture: Awaited<ReturnType<typeof startHostedFixture>> | undefined;
let starting: Promise<Awaited<ReturnType<typeof startHostedFixture>>> | undefined;
let closing: Promise<void> | undefined;
let directory = "", executable = "";
const close = () => closing ??= (async () => {
  await starting?.catch(() => {});
  const games = fixture?.capture() ?? [];
  await fixture?.close();
  for (const game of games) {
    const root = join(directory, game.id);
    if (!existsSync(root)) continue;
    execFileSync(executable, ["lifecycle-join", root], { timeout: 15_000, stdio: "ignore" });
    for (const seat of [1, 2, 3]) if (existsSync(join(root, `bot-${seat}`, "lease")))
      execFileSync("flock", ["--nonblock", join(root, `bot-${seat}`, "lease"), "true"], { timeout: 2_000, stdio: "ignore" });
    privateTree(root);
  }
  process.send?.({ stopped: true, games: games.length, leases: games.length * 3 });
  process.disconnect?.();
})();
process.once("message", async (options: { directory: string; dist: string; inputs: Inputs }) => {
  directory = options.directory; executable = join(options.inputs.nativeRoot, "mtg-server");
  try {
    starting = startHostedFixture(options);
    fixture = await starting;
    if (!process.connected || closing) { await close(); return; }
    process.send?.({ ready: true, origin: fixture.origin, owner: fixture.owner, foreign: fixture.foreign, deckId: fixture.deckId, identity: fixture.identity, cards: fixture.cards });
    process.on("message", () => { void close().catch(() => { process.exitCode = 1; process.disconnect?.(); }); });
  } catch { process.send?.({ failed: true }); process.exitCode = 1; process.disconnect?.(); }
});
process.once("disconnect", () => { void close().catch(() => { process.exitCode = 1; }); });
process.once("SIGTERM", () => { void close().catch(() => { process.exitCode = 1; }); });
