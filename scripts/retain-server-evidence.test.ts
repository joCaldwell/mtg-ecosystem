import assert from 'node:assert/strict';
import test from 'node:test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {fileURLToPath} from 'node:url';
const scripts=path.dirname(fileURLToPath(import.meta.url));
const sha=(bytes:Buffer|string)=>createHash('sha256').update(bytes).digest('hex');
for(const alteration of ['bundle','executable','dependency-manifest','predecessor-manifest','missing-replay-proof','contradictory-replay-proof'] as const){
 test(`server retention rejects altered ${alteration} before executing retained tools`,()=>{
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'mtg-server-retention-guard-'));
  try{
   fs.mkdirSync(path.join(root,'scripts'));fs.mkdirSync(path.join(root,'generated'));const store=path.join(root,'retained');fs.mkdirSync(store);
   for(const name of ['retain-server-evidence.ts','capture-json-output.ts','retain-policy-sources.ts','retention-workspace.ts'])fs.copyFileSync(path.join(scripts,name),path.join(root,'scripts',name));
   const marker=path.join(root,'executed-marker'),executable=path.join(store,'tool.mjs'),bundle=path.join(store,'bundle.json');
   fs.writeFileSync(executable,`#!/usr/bin/env node\nimport fs from 'node:fs';fs.writeFileSync(${JSON.stringify(marker)},'executed');process.stdout.write('{}');\n`,{mode:0o700});
   fs.writeFileSync(bundle,'{}');
   // Prove the marker tool really executes in this fixture; each rejected
   // extension below must stop before it can execute again.
   assert.equal(spawnSync(executable,[],{encoding:'utf8'}).status,0);assert.equal(fs.readFileSync(marker,'utf8'),'executed');fs.unlinkSync(marker);
   const evidence:{verification?:{status:string;state_sha256:string};state_sha256:string;[key:string]:unknown}={state_sha256:'a'.repeat(64),verification:{status:'replay_verified',state_sha256:'a'.repeat(64)},cargo_frozen_offline:true,empty_cargo_home_before_build:true,bundle,bundle_sha256:sha(fs.readFileSync(bundle)),rebuilt_executable:executable,executable_sha256:sha(fs.readFileSync(executable)),dependency_files:{},dependency_manifest_sha256:sha('{}')};
   if(alteration==='missing-replay-proof')delete evidence.verification;
   if(alteration==='contradictory-replay-proof')evidence.verification!.state_sha256='b'.repeat(64);
   if(alteration==='bundle')evidence.bundle_sha256='0'.repeat(64);
   if(alteration==='executable')evidence.executable_sha256='0'.repeat(64);
   if(alteration==='dependency-manifest')evidence.dependency_manifest_sha256='0'.repeat(64);
   const text=JSON.stringify(evidence,null,2)+'\n';fs.writeFileSync(path.join(root,'generated/p2-retention.json'),text);fs.writeFileSync(path.join(store,'evidence.json'),text+(alteration==='predecessor-manifest'?'\n':''));
   const result=spawnSync(process.execPath,[path.join(root,'scripts/retain-server-evidence.ts')],{encoding:'utf8',timeout:5000});
   assert.equal(result.error,undefined);assert.notEqual(result.status,0);assert.equal(fs.existsSync(marker),false);
  }finally{fs.rmSync(root,{recursive:true,force:true});}
 });
}

for(const mutation of ['executable','bundle','manifest'] as const){
 test(`server retention rejects metadata tool mutation of ${mutation} before Cargo`,()=>{
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'mtg-server-retention-mutation-'));
  try{
   const store=path.join(root,'retained'),bin=path.join(root,'bin');
   for(const directory of ['scripts','generated','retained','bin'])fs.mkdirSync(path.join(root,directory));
   for(const name of ['retain-server-evidence.ts','capture-json-output.ts','retain-policy-sources.ts','retention-workspace.ts'])fs.copyFileSync(path.join(scripts,name),path.join(root,'scripts',name));
   const marker=path.join(root,'metadata-ran'),cargoMarker=path.join(root,'cargo-ran');
   const executable=path.join(store,'metadata.mjs'),bundle=path.join(store,'bundle.json'),manifest=path.join(root,'generated/p2-retention.json');
   const mutatedPath=mutation==='executable'?executable:mutation==='bundle'?bundle:manifest;
   const buildID='c'.repeat(64);
   fs.writeFileSync(executable,`#!/usr/bin/env node\nimport fs from 'node:fs';\nif(process.argv[2]!=='bundle-metadata')process.exit(2);\nfs.writeFileSync(${JSON.stringify(marker)},'ran');\nfs.appendFileSync(${JSON.stringify(mutatedPath)},'\\n');\nprocess.stdout.write(${JSON.stringify(JSON.stringify({build_id:buildID}))});\n`,{mode:0o700});
   fs.writeFileSync(path.join(bin,'cargo'),`#!/usr/bin/env node\nimport fs from 'node:fs';fs.writeFileSync(${JSON.stringify(cargoMarker)},'ran');process.exit(1);\n`,{mode:0o700});
   fs.writeFileSync(bundle,'{}');
   const evidence={build_id:buildID,state_sha256:'a'.repeat(64),verification:{status:'replay_verified',state_sha256:'a'.repeat(64)},cargo_frozen_offline:true,empty_cargo_home_before_build:true,bundle,bundle_sha256:sha(fs.readFileSync(bundle)),rebuilt_executable:executable,executable_sha256:sha(fs.readFileSync(executable)),dependency_files:{},dependency_manifest_sha256:sha('{}')};
   const bytes=JSON.stringify(evidence,null,2)+'\n';fs.writeFileSync(manifest,bytes);fs.writeFileSync(path.join(store,'evidence.json'),bytes);
   const result=spawnSync(process.execPath,[path.join(root,'scripts/retain-server-evidence.ts')],{encoding:'utf8',timeout:5000,env:{...process.env,PATH:bin+path.delimiter+process.env.PATH}});
   assert.equal(result.error,undefined);assert.notEqual(result.status,0);
   assert.equal(fs.readFileSync(marker,'utf8'),'ran','real metadata tool must have run');
   assert.equal(fs.existsSync(cargoMarker),false,'mutation must be detected before Cargo');
   const reason=mutation==='executable'?'Predecessor executable changed':mutation==='bundle'?'Native retained bundle changed':'Predecessor manifest changed';
   assert.ok(result.stderr.includes(reason),result.stderr);
  }finally{fs.rmSync(root,{recursive:true,force:true});}
 });
}

// The permission change is exercised against the installed npm CLI, not a mock
// that merely expects the new argument. The only network peer is this test's
// ephemeral loopback tarball server; no registry/global configuration changes.
test('npm12 direct URL cache preparation requires a command-only remote opt-in',async()=>{
 const {createServer}=await import('node:http');
 const {once}=await import('node:events');
 const {spawn}=await import('node:child_process');
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'mtg-npm12-url-'));
 let requests=0;
 const server=createServer((_request,response)=>{
  requests++;response.writeHead(200,{'content-type':'application/octet-stream'});
  response.end(fs.readFileSync(path.join(root,'fixture.tgz')));
 });
 try{
  const pkg=path.join(root,'package');fs.mkdirSync(pkg);
  fs.writeFileSync(path.join(pkg,'package.json'),JSON.stringify({name:'mtg-retention-loopback-fixture',version:'1.0.0',scripts:{prepare:'exit 90',install:'exit 91'}}));
  const packed=spawnSync('tar',['-czf',path.join(root,'fixture.tgz'),'-C',root,'package'],{encoding:'utf8'});
  assert.equal(packed.status,0,packed.stderr);
  server.listen(0,'127.0.0.1');await once(server,'listening');
  const address=server.address();assert.ok(address&&typeof address==='object');
  const url=`http://127.0.0.1:${address.port}/fixture.tgz`;
  const user=path.join(root,'user.npmrc'),global=path.join(root,'global.npmrc');
  fs.writeFileSync(user,'');fs.writeFileSync(global,'');
  const args=['cache','add',url,'--cache',path.join(root,'cache'),`--userconfig=${user}`,`--globalconfig=${global}`,'--ignore-scripts','--no-audit','--no-fund'];
  const run=(extra:string[])=>new Promise<{status:number|null;output:string}>((resolve,reject)=>{
   const child=spawn('npm',[...args,...extra],{cwd:root,env:{PATH:process.env.PATH,LD_LIBRARY_PATH:process.env.LD_LIBRARY_PATH,HOME:root,TMPDIR:root},signal:AbortSignal.timeout(10_000)});
   let output='';child.stdout.on('data',b=>{output+=String(b);});child.stderr.on('data',b=>{output+=String(b);});
   child.on('error',reject);child.on('close',status=>resolve({status,output}));
  });
  const denied=await run([]);assert.notEqual(denied.status,0);assert.match(denied.output,/EALLOWREMOTE/u);assert.equal(requests,0);
  const allowed=await run(['--allow-remote=all']);assert.equal(allowed.status,0,allowed.output);assert.ok(requests>0);
  const source=fs.readFileSync(path.join(scripts,'retain-server-evidence.ts'),'utf8');
  const lines=source.split('\n').filter(line=>line.includes("run('npm'"));
  assert.equal(lines.filter(line=>line.includes('--allow-remote=all')).length,1,'remote opt-in must stay isolated to cache preparation');
  assert.ok(lines.find(line=>line.includes('--allow-remote=all'))!.includes("['cache','add',url"));
  assert.ok(lines.filter(line=>line.includes("['ci'")||line.includes("['run','verify'")).every(line=>line.includes('--offline')&&!line.includes('--allow-remote')));
  assert.equal(fs.readFileSync(user,'utf8'),'');assert.equal(fs.readFileSync(global,'utf8'),'');
 }finally{
  await new Promise<void>(resolve=>{server.close(()=>resolve());});
  fs.rmSync(root,{recursive:true,force:true});
 }
});

for(const mutation of ['none','source-mismatch','changed-copy','extra-fixture'] as const){
 test(`offline SDK repository-relative native fixture layout: ${mutation}`,()=>{
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'mtg-sdk-layout-'));
  try{
   const checkout=path.join(root,'checkout'),source=path.join(root,'native'),evidence=path.join(root,'evidence');
   const fixtureRelative='packages/game-engine/tests/fixtures/expected-match-ir.json';
   const sdk=path.join(checkout,'packages/game-server/sdk');
   fs.mkdirSync(path.join(sdk,'tests'),{recursive:true});fs.mkdirSync(path.dirname(path.join(source,fixtureRelative)),{recursive:true});fs.mkdirSync(evidence);
   const fixtureBytes='{"schema_version":11,"definitions":[{"authentic_fixture":"retained-native"}]}\n';
   fs.writeFileSync(path.join(source,fixtureRelative),fixtureBytes);
   // Same import-meta relationship as the three actual SDK tests that failed
   // in the formerly flat retained layout. The copied test executes for real.
   fs.writeFileSync(path.join(sdk,'tests/probe.mjs'),"import fs from 'node:fs';process.stdout.write(fs.readFileSync(new URL('../../../game-engine/tests/fixtures/expected-match-ir.json',import.meta.url),'utf8'));\n");
   fs.writeFileSync(path.join(sdk,'package.json'),'{"type":"module"}');
   const retainer=fs.readFileSync(path.join(scripts,'retain-server-evidence.ts'),'utf8');
   const helpers=retainer.slice(retainer.indexOf('function files('),retainer.indexOf('function output('));
   const layout=retainer.slice(retainer.indexOf('const clientLayoutVersion='),retainer.indexOf('const lock='));
   assert.ok(helpers.startsWith('function files(')&&layout.startsWith('const clientLayoutVersion='));
   const expectedNative={[fixtureRelative]:mutation==='source-mismatch'?'0'.repeat(64):sha(fixtureBytes)};
   const after=mutation==='changed-copy'?"fs.writeFileSync(fixtureDestination,'tampered');checkClientFixtures();":mutation==='extra-fixture'?"fs.writeFileSync(path.join(path.dirname(fixtureDestination),'extra.json'),'{}');checkClientFixtures();":'';
   const driver=path.join(root,'layout.ts');
   fs.writeFileSync(driver,`import fs from 'node:fs';import path from 'node:path';import {createHash} from 'node:crypto';\nconst root=${JSON.stringify(checkout)},source=${JSON.stringify(source)},evidence=${JSON.stringify(evidence)},expectedNative=${JSON.stringify(expectedNative)};\nconst sha=(bytes:Buffer|string)=>createHash('sha256').update(bytes).digest('hex');\n${helpers}\n${layout}\n${after}\nconsole.log(JSON.stringify({client,clientSource,clientLayoutVersion,clientNativeFixtureFiles}));\n`);
   const result=spawnSync(process.execPath,[driver],{encoding:'utf8',timeout:5000});assert.equal(result.error,undefined);
   if(mutation==='none'){
    assert.equal(result.status,0,result.stderr);const record=JSON.parse(result.stdout);
    assert.equal(record.client,path.join(evidence,'client-source/packages/game-server/sdk'));
    assert.equal(record.clientLayoutVersion,'sdk-repository-relative-fixtures-v1');
    assert.deepEqual(record.clientNativeFixtureFiles,{[fixtureRelative]:sha(fixtureBytes)});
    const probe=spawnSync(process.execPath,[path.join(record.client,'tests/probe.mjs')],{encoding:'utf8',timeout:5000});
    assert.equal(probe.status,0,probe.stderr);assert.equal(probe.stdout,fixtureBytes);
    assert.equal(fs.readFileSync(path.join(evidence,'client-source',fixtureRelative),'utf8'),fixtureBytes);
    assert.equal(fs.existsSync(path.join(root,'game-engine')),false);
   }else{
    assert.notEqual(result.status,0);assert.match(result.stderr,/fixture.*(authority|closure changed)/u);
   }
   assert.equal(fs.readFileSync(path.join(source,fixtureRelative),'utf8'),fixtureBytes,'retained native input must remain immutable');
  }finally{fs.rmSync(root,{recursive:true,force:true});}
 });
}
