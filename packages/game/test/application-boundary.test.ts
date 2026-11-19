import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync, mkdirSync, mkdtempSync, cpSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";
import { isQualifiedProductSession } from "../web/src/session/qualification.ts";

const repo = fileURLToPath(new URL("../../../", import.meta.url));
function sources(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap(row => row.isDirectory() ? sources(join(directory, row.name)) : row.name.endsWith(".ts") ? [join(directory, row.name)] : []);
}
test("deck source has no game/SDK/generated imports; game server uses public deck ports", () => {
  for (const file of sources(join(repo, "packages/deck-builder/src"))) {
    const source = readFileSync(file, "utf8");
    assert.doesNotMatch(source, /(?:from\s*|import\s*\()\s*["'][^"']*(?:@multiverse\/game|game-server|\/generated\/)/, file);
  }
  for (const file of sources(join(repo, "packages/game/server"))) {
    const source = readFileSync(file, "utf8");
    assert.doesNotMatch(source, /["'][^"']*(?:deck-builder\/src|deck-builder\/(?!application["']))/, file);
  }
});
test("legacy browser exports preserve the clean public declaration surface", () => {
  // Resolve only type declarations; no generated SDK inputs or implementation
  // imports are needed by a host consuming either public package name.
  const modern = JSON.parse(readFileSync(join(repo, "packages/game/package.json"), "utf8"));
  const legacy = JSON.parse(readFileSync(join(repo, "packages/game-client/package.json"), "utf8"));
  for (const name of ["preview", "session", "session-qualification"]) {
    assert.ok(legacy.exports[`./${name}`]);
    const declaration = readFileSync(join(repo, "packages/game", modern.exports[`./${name}`].types), "utf8");
    assert.doesNotMatch(declaration, /generated|game-server-sdk|server\//);
  }
  const output = execFileSync(process.execPath, ["--input-type=module", "-e", "import assert from 'node:assert/strict'; import { isQualifiedProductSession as a } from '@multiverse/game/session-qualification'; import { isQualifiedProductSession as b } from '@multiverse/game-client/session-qualification'; assert.equal(a,b); assert.equal(a(undefined),false)"], { cwd: join(repo, "packages/game-client"), encoding: "utf8" });
  assert.equal(output, "");
});

test("product launch admits the exact qualified foundations identity", () => {
  const { engine_build_id, server_build_id } = JSON.parse(readFileSync(join(repo, "packages/game/sdk-qualification.json"), "utf8"));
  const identity = { profile: "commander-foundations-v1", catalog_sha256: "ff307bd50c7714f3ad99c1bd558217cd38b0223c1fe48dfd200d2b611abc8b56", engine_build_id, server_build_id };
  assert.equal(isQualifiedProductSession(identity), true);
  assert.equal(isQualifiedProductSession(undefined), false);
  for (const field of Object.keys(identity)) {
    assert.equal(isQualifiedProductSession({ ...identity, [field]: "different" }), false, field);
  }
});

test("both public package names typecheck in an isolated consumer with no SDK or generated files", () => {
  const consumer = mkdtempSync(join(tmpdir(), "game-clean-consumer-"));
  const modules = join(consumer, "node_modules");
  const modern = JSON.parse(readFileSync(join(repo, "packages/game/package.json"), "utf8"));
  const legacy = JSON.parse(readFileSync(join(repo, "packages/game-client/package.json"), "utf8"));
  for (const [name, pkg] of [["game", modern], ["game-client", legacy]] as const) {
    const destination = join(modules, "@multiverse", name);
    mkdirSync(destination, { recursive: true });
    cpSync(join(repo, "packages", name, "package.json"), join(destination, "package.json"));
    for (const subpath of ["preview", "session", "session-qualification", "verify-preview"]) {
      const entry = pkg.exports[`./${subpath}`];
      const file = typeof entry === "string" ? entry : entry.types;
      mkdirSync(dirname(join(destination, file)), { recursive: true });
      cpSync(join(repo, "packages", name, file), join(destination, file));
    }
  }
  mkdirSync(join(modules, "@types"), { recursive: true });
  symlinkSync(join(repo, "packages/game/node_modules/@types/react"), join(modules, "@types/react"));
  symlinkSync(join(repo, "packages/game/node_modules/csstype"), join(modules, "csstype"));
  writeFileSync(join(consumer, "consumer.ts"), `import { OpeningTablePreview } from '@multiverse/game/preview';
import { GameSession } from '@multiverse/game/session';
import { GameSession as LegacySession } from '@multiverse/game-client/session';
import { OpeningTablePreview as LegacyPreview } from '@multiverse/game-client/preview';
import { isQualifiedProductSession } from '@multiverse/game-client/session-qualification';
import { verifyPreviewFixtures } from '@multiverse/game-client/verify-preview';
void verifyPreviewFixtures;
const session: typeof GameSession = LegacySession; const preview: typeof OpeningTablePreview = LegacyPreview;
void [session, preview, isQualifiedProductSession(undefined)];
`);
  execFileSync(process.execPath, [join(repo, "packages/game/node_modules/typescript/bin/tsc"), "--noEmit", "--strict", "--skipLibCheck", "--target", "ES2023", "--module", "ESNext", "--moduleResolution", "Bundler", "--allowImportingTsExtensions", "--types", "react", "consumer.ts"], { cwd: consumer, encoding: "utf8" });
});
