import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { retainExecutable, retentionScratch } from './retention-workspace.ts';

test('copied executable survives scratch deletion with exact bytes and mode', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'retention-copy-'));
  try {
    const scratch = path.join(root, 'scratch'); fs.mkdirSync(scratch);
    const source = path.join(scratch, 'worker');
    fs.writeFileSync(source, '#!/bin/sh\nprintf verified', { mode: 0o700 });
    const retained = retainExecutable(source, path.join(root, 'evidence'), 'worker');
    assert.deepEqual(fs.readFileSync(retained), fs.readFileSync(source));
    assert.equal(fs.statSync(retained).mode, fs.statSync(source).mode);
    fs.rmSync(scratch, { recursive: true });
    const result = spawnSync(retained, [], { encoding: 'utf8' });
    assert.equal(result.status, 0); assert.equal(result.stdout, 'verified');
    assert.throws(() => retainExecutable(retained, path.join(root, 'evidence'), 'worker'), /EEXIST/);
    assert.throws(() => retainExecutable(retained, path.join(root, 'evidence'), '../escape'), /Invalid/);
    assert.throws(() => retainExecutable(retained, path.join(root, 'evidence'), '..'), /Invalid/);
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
});

test('retained executable output refuses symlink escape', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'retention-link-'));
  try {
    const store = path.join(root, 'store'), outside = path.join(root, 'outside');
    fs.mkdirSync(store); fs.mkdirSync(outside);
    fs.symlinkSync(outside, path.join(store, 'bin'));
    const source = path.join(root, 'source'); fs.writeFileSync(source, 'unique');
    assert.throws(() => retainExecutable(source, store, 'worker'), /symlink/);
    assert.deepEqual(fs.readdirSync(outside), []);
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
});

test('worker requires the live parent supervisor ownership marker', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'retention-owner-'));
  const previous = process.env.MTG_RETENTION_SCRATCH;
  try {
    delete process.env.MTG_RETENTION_SCRATCH;
    assert.throws(() => retentionScratch(), /bounded retention/);
    process.env.MTG_RETENTION_SCRATCH = root;
    fs.writeFileSync(path.join(root, 'owner.json'), JSON.stringify({ version: 1, pid: -1 }));
    assert.throws(() => retentionScratch(), /ownership/);
    fs.writeFileSync(path.join(root, 'owner.json'), JSON.stringify({ version: 1, pid: process.ppid }));
    assert.equal(retentionScratch(), root);
  } finally {
    if (previous === undefined) delete process.env.MTG_RETENTION_SCRATCH;
    else process.env.MTG_RETENTION_SCRATCH = previous;
    fs.rmSync(root, { recursive: true, force: true });
  }
});
