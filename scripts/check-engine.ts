import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const version = spawnSync("rustc", ["--version"], { encoding: "utf8" });
if (version.status !== 0 || !version.stdout.startsWith("rustc 1.99.0 ")) {
  console.error("Engine checks require the pinned Rust 1.99.0 toolchain.");
  process.exit(1);
}
for (const [command, args] of [
  ["cargo", ["fmt", "--all", "--", "--check"]],
  ["cargo", ["clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"]],
  ["cargo", ["test", "--workspace", "--locked"]],
  ["node", ["--test", "packages/game-engine/tests/contracts.test.ts"]],
] as const) {
  const result = spawnSync(command, [...args], { cwd: root, stdio: "inherit" });
  if (result.status !== 0) process.exit(result.status ?? 1);
}
