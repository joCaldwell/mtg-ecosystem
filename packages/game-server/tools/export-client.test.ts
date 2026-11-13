import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, symlinkSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { exportClient } from './export-client.ts';
import { sha256 } from './client-artifacts.ts';
test('export copies only explicitly named public assets and pins exact bytes', () => {
 const dir=mkdtempSync(join(tmpdir(),'client-export-'));
 try {
  const dist=join(dir,'dist'); mkdirSync(dist); mkdirSync(join(dist,'assets'));
  writeFileSync(join(dist,'index.html'),'<script src="/assets/app.js"></script>'); writeFileSync(join(dist,'assets/app.js'),'export {};');
  writeFileSync(join(dist,'operator.json'),'PRIVATE');
  const out=join(dir,'public'); const result=exportClient(dist,out,['index.html','assets/app.js']);
  assert.equal(result.bundle_sha256,sha256(readFileSync(join(out,'client-assets.json')))); assert.equal(existsSync(join(out,'operator.json')),false);
  for (const path of ['operator.json','../operator.json','assets/../operator.json','assets/app.js.map','assets/key.db','assets/a..js']) assert.throws(()=>exportClient(dist,join(dir,'bad'),['index.html',path]));
  symlinkSync(join(dist,'assets/app.js'),join(dist,'assets/link.js'));
  assert.throws(()=>exportClient(dist,join(dir,'bad'),['index.html','assets/link.js'])); assert.equal(existsSync(join(dir,'bad')),false);
  assert.throws(()=>exportClient(dist,out,['index.html']));
 } finally {rmSync(dir,{recursive:true,force:true});}
});
