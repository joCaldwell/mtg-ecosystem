import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createServer } from "node:http";
import { createHost } from "../src/host.ts";

async function fixture() {
  const directory = await mkdtemp(join(tmpdir(), "multiverse-host-"));
  const webDist = join(directory, "web"); await mkdir(webDist);
  await writeFile(join(webDist, "index.html"), "<!doctype html><title>Host fixture</title>");
  return { directory, options: { webDist, databasePath: join(directory, "account.db"), dataDirectory: join(directory, "data"), backgroundJobs: false, isApplicationPath: (path: string) => path === "/" || path === "/product-fixture", mode: "local" as const, port: 0 } };
}

test("public host serves a product bundle and account API with one disposable database", async () => {
  const { directory, options } = await fixture();
  try {
    const host = await createHost(options);
    const first = await host.start(); assert.deepEqual(await host.start(), first);
    const origin = `http://127.0.0.1:${first.port}`;
    assert.equal((await fetch(origin + "/api/session").then(r => r.json())).mode, "local");
    assert.match(await fetch(origin).then(r => r.text()), /Host fixture/);
    assert.match(await fetch(origin + "/product-fixture").then(r => r.text()), /Host fixture/);
    assert.equal((await fetch(origin + "/play")).status, 404);
    assert.equal(host.paths.database, options.databasePath);
    await host.close(); await host.close();
    const successor = await createHost(options); await successor.start(); await successor.close();
  } finally { await rm(directory, { recursive: true, force: true }); }
});

test("listener failure closes the public host and releases its database lease", async () => {
  const { directory, options } = await fixture();
  const busy = createServer(); await new Promise<void>(resolve => busy.listen(0, "127.0.0.1", resolve));
  try {
    const address = busy.address(); assert.ok(address && typeof address !== "string");
    const host = await createHost({ ...options, port: address.port });
    await assert.rejects(host.start(), /EADDRINUSE/);
    const successor = await createHost(options); await successor.start(); await successor.close();
  } finally { await new Promise<void>(resolve => busy.close(() => resolve())); await rm(directory, { recursive: true, force: true }); }
});

test("closing during listener startup settles both callers and releases the lease", { timeout: 2_000 }, async () => {
  const { directory, options } = await fixture();
  try {
    const host = await createHost(options);
    const starting = host.start();
    const closed = host.close();
    await Promise.all([assert.rejects(starting, /host is closed/), closed]);
    assert.equal(host.server.listening, false);
    await assert.rejects(host.start(), /host is closed/);
    const successor = await createHost(options);
    await successor.start(); await successor.close();
  } finally { await rm(directory, { recursive: true, force: true }); }
});
