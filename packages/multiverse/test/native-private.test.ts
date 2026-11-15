import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import { syncBuiltinESMExports } from "node:module";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { privateDirectory } from "../src/native-private.ts";

const fsync = fs.fsyncSync;

test("privateDirectory syncs the directory then its parent, including retries", t => {
  const parent = fs.mkdtempSync(join(tmpdir(), "multiverse-directory-sync-"));
  const path = join(parent, "game");
  const synced: string[] = [];
  t.mock.method(fs, "fsyncSync", (fd: number) => {
    synced.push(fs.readlinkSync("/proc/self/fd/" + fd));
    fsync(fd);
  });
  syncBuiltinESMExports();
  try {
    privateDirectory(path);
    assert.deepEqual(synced, [path, parent]);
    assert.equal(fs.lstatSync(path).mode & 0o777, 0o700);
    synced.length = 0;
    privateDirectory(path);
    assert.deepEqual(synced, [path, parent]);
  } finally { t.mock.restoreAll(); syncBuiltinESMExports(); }
});

for (const failAt of [0, 1]) {
  test("privateDirectory propagates sync failure at index " + failAt + " and resyncs on retry", t => {
    const parent = fs.mkdtempSync(join(tmpdir(), "multiverse-directory-sync-failure-"));
    const path = join(parent, "game");
    const synced: string[] = [], descriptors: number[] = [];
    const failure = new Error("directory sync failed");
    t.mock.method(fs, "fsyncSync", (fd: number) => {
      synced.push(fs.readlinkSync("/proc/self/fd/" + fd));
      descriptors.push(fd);
      if (synced.length === failAt + 1) throw failure;
      fsync(fd);
    });
    syncBuiltinESMExports();
    try {
      assert.throws(() => privateDirectory(path), error => error === failure);
      assert.deepEqual(synced, [path, parent].slice(0, failAt + 1));
      for (const fd of descriptors) assert.throws(() => fs.fstatSync(fd), { code: "EBADF" });
    } finally { t.mock.restoreAll(); syncBuiltinESMExports(); }
    // Creation survived the error in this process; retry must still durably
    // establish the parent link rather than trusting the existing directory.
    const retried: string[] = [];
    t.mock.method(fs, "fsyncSync", (fd: number) => {
      retried.push(fs.readlinkSync("/proc/self/fd/" + fd)); fsync(fd);
    });
    syncBuiltinESMExports();
    try { privateDirectory(path); assert.deepEqual(retried, [path, parent]); }
    finally { t.mock.restoreAll(); syncBuiltinESMExports(); }
  });
}
