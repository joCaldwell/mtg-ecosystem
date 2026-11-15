import test from "node:test";
import assert from "node:assert/strict";
import { cpSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, symlinkSync, writeFileSync, unlinkSync } from "node:fs";
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { dirname, join, matchesGlob, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { tmpdir } from "node:os";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const dockerfile = readFileSync(join(repo, "packages/deck-builder/Dockerfile"), "utf8");
const rules = readFileSync(join(repo, ".dockerignore"), "utf8").split("\n").filter(row => row && !row.startsWith("#"));
function included(path: string) {
  let include = true;
  for (const row of rules) {
    const negated = row.startsWith("!"), pattern = (negated ? row.slice(1) : row).replace(/\/$/, "/**");
    if (matchesGlob(path, pattern)) include = negated;
  }
  return include;
}
function files(root: string): string[] {
  return readdirSync(root).flatMap(name => {
    const file = join(root, name), stat = lstatSync(file);
    assert.ok(!stat.isSymbolicLink(), `Unexpected source symlink: ${file}`);
    return stat.isDirectory() ? files(file) : [file];
  });
}

test("Docker context and final image carry the public SDK closure and live session build inputs", () => {
  for (const path of ["packages/game-server/sdk/dist/browser.js", "packages/game-server/sdk/dist/presentation-producer.js", "packages/game-server/sdk/dist/presentation.schema.json", "packages/game-server/sdk/src/policies.ts", "packages/game-server/sdk/src/examples/foundations-consumer.ts", "packages/game-server/sdk/generated/validators.js", "packages/game-server/sdk/generated/routes.ts", "packages/game-server/sdk/generated/generation.json", "packages/game-server/sdk/generated/verification.json", "packages/game-client/src/app/session.ts", "packages/game-client/generated/sdk-identity.ts", "packages/multiverse/scripts/check-sdk-runtime.mjs"]) assert.ok(included(path), `Excluded runtime/build input: ${path}`);
  for (const path of [".env", "packages/deck-builder/data/account.db", "packages/game-server/sdk/node_modules/ajv/index.js", "packages/game-server/sdk/generated/verification.log", "packages/game-server/tools/client-artifacts.ts", "packages/game-engine/src/main.rs", "packages/multiverse/web/dist/assets/old.js"]) assert.ok(!included(path), `Unexpected context input: ${path}`);
  const runtime = dockerfile.split(" AS runtime\n")[1]!;
  for (const directory of ["dist", "src", "generated"]) assert.ok(runtime.includes(`COPY --from=build /app/packages/game-server/sdk/${directory} ./packages/game-server/sdk/${directory}`));
  assert.ok(runtime.includes("node packages/multiverse/scripts/check-sdk-runtime.mjs"));
  assert.ok(!/^RUN.*(?:cargo|sdk.*(?:run generate|build:browser))/m.test(dockerfile));
});

test("isolated production layout imports verified SDK exports and cold-starts the actual native-disabled server", { skip: process.env.MULTIVERSE_TEST_CONTAINER_RUNTIME !== "1", timeout: 120_000 }, async () => {
  const evidence = mkdtempSync(join(process.env.MULTIVERSE_TEST_CONTAINER_EVIDENCE_DIRECTORY ?? tmpdir(), "product-container-runtime-"));
  const context = join(evidence, "context"), runtime = join(evidence, "runtime");
  mkdirSync(context); mkdirSync(runtime);
  const env = { PATH: process.env.PATH, ...(process.env.LD_LIBRARY_PATH ? { LD_LIBRARY_PATH: process.env.LD_LIBRARY_PATH } : {}), NODE_ENV: "production" };
  const candidates = execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z"], { cwd: repo, encoding: "utf8" }).split("\0").filter(Boolean);
  for (const directory of ["packages/game-server/sdk/dist", "packages/game-server/sdk/generated", "packages/game-client/generated"]) candidates.push(...files(join(repo, directory)).map(file => relative(repo, file)));
  for (const path of new Set(candidates.filter(included))) { mkdirSync(dirname(join(context, path)), { recursive: true }); cpSync(join(repo, path), join(context, path)); }
  // Build dependencies are copied from existing local locked installs; no npm
  // install, source generation, native command or network bootstrap occurs.
  for (const pkg of ["deck-builder", "multiverse"]) cpSync(join(repo, "packages", pkg, "node_modules"), join(context, "packages", pkg, "node_modules"), { recursive: true, verbatimSymlinks: true });
  // Root workspace links provided by Docker's existing npm-ci stage. Local
  // isolated package installs put these links elsewhere, so recreate them here.
  for (const [name, target] of [["@mtg-ecosystem/deck-builder", "deck-builder"], ["@multiverse/app", "multiverse"], ["@multiverse/game-client", "game-client"], ["@multiverse/game-ui", "game-ui"], ["@multiverse/game-server-sdk", "game-server/sdk"]]) {
    const link = join(context, "node_modules", name!); mkdirSync(dirname(link), { recursive: true }); symlinkSync(relative(dirname(link), join(context, "packages", target!)), link);
  }
  const run = (args: string[], cwd = context) => execFileSync(process.execPath, args, { cwd, env, encoding: "utf8", maxBuffer: 8 * 1024 * 1024 });
  writeFileSync(join(evidence, "context-sdk.log"), run(["packages/multiverse/scripts/check-sdk-runtime.mjs"]));
  writeFileSync(join(evidence, "context-build.log"), run(["node_modules/vite/bin/vite.js", "build"], join(context, "packages/multiverse")));
  for (const pkg of ["deck-builder", "multiverse"]) run(["packages/deck-builder/scripts/check-runtime.mjs", "--manifest", "--package", `packages/${pkg}`]);
  // Materialize the Dockerfile's final COPY instructions from the isolated
  // build context, substituting existing locked production dependencies below.
  for (const line of dockerfile.split(" AS runtime\n")[1]!.split("\n")) {
    const copy = line.match(/^COPY (?:--from=\S+ )?(\S+) (\S+)$/);
    if (!copy || copy[1]!.includes("node_modules")) continue;
    const source = join(context, copy[1]!.replace(/^\/app\//, ""));
    const target = join(runtime, copy[2]!.replace(/^\.\//, "").replace(/^\//, ""));
    mkdirSync(dirname(target), { recursive: true }); cpSync(source, target, { recursive: true });
  }
  for (const pkg of ["deck-builder", "multiverse"]) {
    const lock = JSON.parse(readFileSync(join(repo, "packages", pkg, "package-lock.json"), "utf8"));
    for (const [path, value] of Object.entries(lock.packages) as [string, { dev?: boolean; link?: boolean }][]) {
      if (!path.startsWith("node_modules/") || value.dev || value.link) continue;
      const target = join(runtime, "packages", pkg, path);
      if (!existsSync(target)) { mkdirSync(dirname(target), { recursive: true }); cpSync(join(context, "packages", pkg, path), target, { recursive: true }); }
    }
  }
  for (const [name, target] of [["@mtg-ecosystem/deck-builder", "deck-builder"], ["@multiverse/game-server-sdk", "game-server/sdk"]]) {
    const link = join(runtime, "node_modules", name!); mkdirSync(dirname(link), { recursive: true }); symlinkSync(relative(dirname(link), join(runtime, "packages", target!)), link);
  }
  assert.ok(!existsSync(join(runtime, "packages/game-server/sdk/node_modules")));
  assert.ok(!existsSync(join(runtime, "packages/game-server/tools")));
  assert.ok(!existsSync(join(runtime, "packages/deck-builder/node_modules/typescript")));
  const sandbox = ["--permission", `--allow-fs-read=${runtime}`, `--allow-fs-write=${runtime}`, "--allow-net"];
  const checked = run([...sandbox, "packages/multiverse/scripts/check-sdk-runtime.mjs"], runtime);
  writeFileSync(join(evidence, "runtime-sdk.log"), checked);
  for (const pkg of ["deck-builder", "multiverse"]) writeFileSync(join(evidence, `${pkg}-runtime.log`), run([...sandbox, "packages/deck-builder/scripts/check-runtime.mjs", "--package", `packages/${pkg}`], runtime));
  for (const path of ["dist/presentation.schema.json", "generated/validators.js"]) {
    const file = join(runtime, "packages/game-server/sdk", path), bytes = readFileSync(file); unlinkSync(file);
    const broken = spawnSync(process.execPath, [...sandbox, "packages/multiverse/scripts/check-sdk-runtime.mjs"], { cwd: runtime, env, encoding: "utf8" });
    writeFileSync(join(evidence, `missing-${path.replaceAll("/", "-")}.log`), broken.stdout + broken.stderr);
    assert.notEqual(broken.status, 0, `Missing ${path} must fail in a fresh process`); writeFileSync(file, bytes);
  }
  const data = join(runtime, "disposable-data");
  assert.ok(!existsSync(data));
  const child = spawn(process.execPath, [...sandbox, "../multiverse/src/server.ts"], { cwd: join(runtime, "packages/deck-builder"), env: { ...env, DECKBUILDER_MODE: "local", DECKBUILDER_DATA: data, DECKBUILDER_PORT: "0" }, stdio: ["ignore", "pipe", "pipe"] });
  let stdout = "", stderr = "";
  child.stdout.on("data", chunk => { stdout += chunk; }); child.stderr.on("data", chunk => { stderr += chunk; });
  const closed = new Promise<number | null>(resolve => child.once("close", code => resolve(code)));
  try {
    const started = await new Promise<{ port: number }>((resolve, reject) => {
      const timer = setTimeout(() => reject(Error("Production cold-start deadline")), 15_000);
      child.once("error", reject); child.once("exit", () => { clearTimeout(timer); reject(Error("Production cold-start exited")); });
      child.stdout.on("data", () => { for (const line of stdout.split("\n")) try { const value = JSON.parse(line); if (value.event === "application_started") { clearTimeout(timer); resolve(value); } } catch {} });
    });
    const base = `http://127.0.0.1:${started.port}`;
    for (const path of ["/api/health", "/api/session", "/api/games", "/"]) {
      const response = await fetch(base + path); assert.equal(response.status, 200, path); await response.arrayBuffer();
    }
    assert.ok(existsSync(join(data, "deck-builder.db")));
    child.kill("SIGTERM"); assert.equal(await closed, 0);
    writeFileSync(join(evidence, "report.json"), JSON.stringify({ result: "passed", native: "disabled", context, runtime, sdk: JSON.parse(checked), actual_entrypoint: "packages/multiverse/src/server.ts", filesystem_reads_confined_to_runtime: true, background_children: "denied by Node permission boundary; no refresh/download", http: ["health", "session", "games", "product"], negative_missing_files: ["presentation.schema.json", "validators.js"] }, null, 2) + "\n");
  } finally {
    if (child.exitCode === null && child.signalCode === null) child.kill("SIGKILL"); await closed;
    writeFileSync(join(evidence, "server.stdout.log"), stdout); writeFileSync(join(evidence, "server.stderr.log"), stderr);
    console.log(`Retained production-layout evidence: ${evidence}`);
  }
});
