// P8 extends an already verified P2 native retention with an independent offline
// server build and a separate locked client closure. Native source authority
// remains the exact bundle source manifest; SDK sources never enter that hash.
// https://docs.npmjs.com/cli/v12/commands/npm-ci (offline, integrity, ignore-scripts)
// https://docs.npmjs.com/cli/v12/commands/npm-cache
// The retained cache is a byte-pinned independent copy; a successful fresh
// offline install, rather than the presence of cache indexes, proves closure.
// https://doc.rust-lang.org/cargo/commands/cargo-vendor.html
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {captureJsonOutput} from './capture-json-output.ts';
import {verifyRetainedPolicySources} from './retain-policy-sources.ts';
const root=fileURLToPath(new URL('../',import.meta.url));
const sha=(bytes:Buffer|string)=>createHash('sha256').update(bytes).digest('hex');
function run(command:string,args:string[],cwd:string,env:NodeJS.ProcessEnv,log?:string):string {
 const r=spawnSync(command,args,{cwd,env,encoding:'utf8',maxBuffer:32*1024*1024});
 if(log)fs.writeFileSync(log,(r.stdout??'')+(r.stderr??''));
 if(r.error||r.status!==0)throw new Error(`${command} verification failed; see retained log`);
 return r.stdout;
}
function files(dir:string,ignoreRoot:ReadonlySet<string>=new Set()):Record<string,string> {
 const manifest:Record<string,string>={};
 function visit(current:string){for(const entry of fs.readdirSync(current,{withFileTypes:true}).sort((a,b)=>a.name.localeCompare(b.name))){
  if(current===dir&&ignoreRoot.has(entry.name))continue;
  const p=path.join(current,entry.name);if(entry.isSymbolicLink())throw new Error('Retained source contains a symlink');
  if(entry.isDirectory())visit(p);else if(entry.isFile())manifest[path.relative(dir,p)]=sha(fs.readFileSync(p));else throw new Error('Retained source contains non-file');
 }}visit(dir);return manifest;
}
function check(dir:string,manifest:Record<string,string>){const actual=files(dir);if(Object.keys(actual).length!==Object.keys(manifest).length||Object.entries(manifest).some(([p,h])=>actual[p]!==h))throw new Error('Retained bytes changed');}
function output(file:string,value:unknown){fs.writeFileSync(file,JSON.stringify(value,null,2)+'\n');}
const predecessorManifest=path.join(root,'generated/p2-retention.json');
const predecessorBytes=fs.readFileSync(predecessorManifest);
const engine=JSON.parse(predecessorBytes.toString('utf8'));
if(engine.cargo_frozen_offline!==true||engine.empty_cargo_home_before_build!==true
 ||engine.verification?.status!=='replay_verified'||typeof engine.state_sha256!=='string'
 ||!/^[0-9a-f]{64}$/.test(engine.state_sha256)||engine.verification.state_sha256!==engine.state_sha256)throw new Error('Verified predecessor replay required');
const store=path.dirname(engine.bundle);
function predecessorPins(){
 if(!fs.readFileSync(predecessorManifest).equals(predecessorBytes)||!fs.readFileSync(path.join(store,'evidence.json')).equals(predecessorBytes))throw new Error('Predecessor manifest changed');
 if(sha(fs.readFileSync(engine.bundle))!==engine.bundle_sha256)throw new Error('Native retained bundle changed');
 if(sha(fs.readFileSync(engine.rebuilt_executable))!==engine.executable_sha256)throw new Error('Predecessor executable changed');
}
predecessorPins();
if(sha(JSON.stringify(engine.dependency_files))!==engine.dependency_manifest_sha256)throw new Error('Predecessor dependency manifest digest differs');
const native=captureJsonOutput<any>(engine.rebuilt_executable,['bundle-metadata',engine.bundle],{cwd:store});
predecessorPins();
if(native.build_id!==engine.build_id)throw new Error('Predecessor compiled identity differs');
function authoritativeSources(){if(sha(fs.readFileSync(engine.rules_path))!==engine.rules_sha256||engine.rules_sha256!==native.catalog.rules_sha256)throw new Error('Retained official rules differ');verifyRetainedPolicySources(native.source_files,engine.policy_sources??[],store);}
authoritativeSources();
const source=path.join(store,'source');
if(!fs.existsSync(path.join(source,'packages/game-server/Cargo.toml')))throw new Error('Retained native closure does not contain server');
const evidence=fs.mkdtempSync(path.join(store,'server-'));
const home=path.join(evidence,'cargo-home'),target=path.join(evidence,'target');fs.mkdirSync(home);
const env={...process.env,CARGO_HOME:home,CARGO_TARGET_DIR:target,CARGO_NET_OFFLINE:'true',CARGO_ENCODED_RUSTFLAGS:native.rustflags,CARGO_PROFILE_RELEASE_OPT_LEVEL:native.opt_level,CARGO_PROFILE_RELEASE_DEBUG:native.debug,CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS:native.debug_assertions,CARGO_BUILD_JOBS:'2'};
// The predecessor already pins all Cargo/vendor bytes; independently recheck it
// before and after compiling the server from the same retained workspace.
const vendor=path.join(store,'vendor');check(vendor,engine.dependency_files);
const sourceBefore=files(source);
const expectedNative:Record<string,string>={};
for(const row of native.source_files){if(path.isAbsolute(row.path)||row.path.split(/[\\/]/u).includes('..')||sha(row.contents)!==row.sha256)throw new Error('Native manifest source row invalid');expectedNative[row.path]=row.sha256;}
if(Object.keys(sourceBefore).length!==Object.keys(expectedNative).length+1||Object.entries(expectedNative).some(([p,h])=>sourceBefore[p]!==h)||!sourceBefore['.cargo/config.toml'])throw new Error('Native source closure differs');
const metadata=JSON.parse(run('cargo',['metadata','--frozen','--offline','--format-version=1'],source,env));
const packages=metadata.packages.filter((p:{source:string|null})=>p.source!==null).map((p:{name:string;version:string;source:string})=>({name:p.name,version:p.version,source:p.source}));
run('cargo',['build','--frozen','--offline','--release','--target',native.target,'-p','mtg-server'],source,env,path.join(evidence,'server-build.log'));
const host=native.target;
const binary=path.join(target,host,'release/mtg-server');
run('cargo',['test','--frozen','--offline','--release','--target',native.target,'-p','mtg-server'],source,env,path.join(evidence,'server-offline-tests.log'));
const binarySha=sha(fs.readFileSync(binary));
const schema=JSON.parse(run(binary,['schema'],evidence,env));
const nativeInfo=JSON.parse(run(binary,['native-info'],evidence,env));
const referenceBinary=process.env.MTG_SERVER_BIN??path.join(root,'target/release/mtg-server');
const referenceSha=sha(fs.readFileSync(referenceBinary));
const referenceInfo=JSON.parse(run(referenceBinary,['native-info'],root,process.env));
function serverPins(){if(sha(fs.readFileSync(binary))!==binarySha||sha(fs.readFileSync(referenceBinary))!==referenceSha)throw new Error('Server executable changed during verification');}
serverPins();predecessorPins();
if(JSON.stringify(referenceInfo)!==JSON.stringify(nativeInfo))throw new Error('Server build/SQLite options differ from verified reference');
if(nativeInfo.engine_build_id!==engine.build_id||nativeInfo.target!==native.target||nativeInfo.rustflags!==native.rustflags||nativeInfo.opt_level!==native.opt_level)throw new Error('Rebuilt server identity differs');
check(source,sourceBefore);check(vendor,engine.dependency_files);authoritativeSources();
// Preserve repository-relative SDK test imports. Only this native-authority
// fixture is needed outside the SDK; never copy or invent a parallel catalog.
const clientLayoutVersion='sdk-repository-relative-fixtures-v1';
const clientLayoutRoot=path.join(evidence,'client-source');
const sdk=path.join(root,'packages/game-server/sdk'),client=path.join(clientLayoutRoot,'packages/game-server/sdk');fs.mkdirSync(client,{recursive:true});
const fixtureRelative='packages/game-engine/tests/fixtures/expected-match-ir.json';
const fixtureSource=path.join(source,fixtureRelative),fixtureDestination=path.join(clientLayoutRoot,fixtureRelative);
const fixtureSha=expectedNative[fixtureRelative];
if(!fixtureSha||sha(fs.readFileSync(fixtureSource))!==fixtureSha)throw new Error('SDK fixture differs from retained native authority');
fs.mkdirSync(path.dirname(fixtureDestination),{recursive:true});fs.copyFileSync(fixtureSource,fixtureDestination);
const clientNativeFixtureFiles:Record<string,string>={[fixtureRelative]:fixtureSha};
function checkClientFixtures(){
 const actual=files(path.join(clientLayoutRoot,'packages/game-engine'));
 const expected={'tests/fixtures/expected-match-ir.json':fixtureSha};
 if(Object.keys(actual).length!==1||Object.entries(expected).some(([p,h])=>actual[p]!==h))throw new Error('Client native fixture closure changed');
}
checkClientFixtures();
function copySDK(current:string,relative=''){
 for(const entry of fs.readdirSync(current,{withFileTypes:true})){
  if(relative===''&&['node_modules','generated','.git'].includes(entry.name))continue;
  const rel=path.join(relative,entry.name),src=path.join(current,entry.name),dst=path.join(client,rel);
  if(entry.isSymbolicLink())throw new Error('SDK source symlink');
  if(entry.isDirectory()){fs.mkdirSync(dst);copySDK(src,rel);}else if(entry.isFile())fs.copyFileSync(src,dst);else throw new Error('SDK source entry');
 }
}copySDK(sdk);const clientSource=files(client);
const lock=JSON.parse(fs.readFileSync(path.join(client,'package-lock.json'),'utf8'));
const preparation=path.join(evidence,'npm-preparation-cache'),cache=path.join(evidence,'npm-offline-cache'),npmHome=path.join(evidence,'npm-home');fs.mkdirSync(npmHome);
// No inherited npm credentials/configuration are needed for this public closure.
const npmEnv:NodeJS.ProcessEnv={PATH:process.env.PATH,HOME:npmHome,TMPDIR:process.env.TMPDIR??'/tmp',MTG_SERVER_BIN:binary};
const userConfig=path.join(npmHome,'user.npmrc'),globalConfig=path.join(npmHome,'global.npmrc');fs.writeFileSync(userConfig,'');fs.writeFileSync(globalConfig,'');
const npmFlags=[`--userconfig=${userConfig}`,`--globalconfig=${globalConfig}`,'--ignore-scripts','--no-audit','--no-fund'];
const resolved=new Set<string>();
for(const [name,value] of Object.entries(lock.packages) as [string,{resolved?:string;integrity?:string}][]){
 if(!name)continue;if(!value.resolved?.startsWith('https://registry.npmjs.org/')||!value.integrity?.startsWith('sha512-'))throw new Error('SDK requires pinned public tarball integrity');
 resolved.add(value.resolved);
}
// npm12 defaults allow-remote to none, including direct URL cache-add commands.
// Opt in ONLY for these already registry/lock-integrity-validated tarballs;
// offline ci and verification below retain their explicit --offline boundary.
// npm12 primary definition: @npmcli/config/lib/definitions/definitions.js allow-remote.
let index=0;for(const url of [...resolved].sort())run('npm',['cache','add',url,'--allow-remote=all','--cache',preparation,...npmFlags],evidence,npmEnv,path.join(evidence,`npm-prepare-${index++}.log`));
const cacheManifest=files(preparation);fs.cpSync(preparation,cache,{recursive:true});check(cache,cacheManifest);
run('npm',['ci','--offline','--cache',cache,'--workspaces=false',...npmFlags],client,npmEnv,path.join(evidence,'npm-offline-install.log'));
// verify invokes generation, strict type checks, protocol runtime tests and its
// own exact source/tool/declaration manifest using this rebuilt Rust executable.
run('npm',['run','verify','--workspaces=false','--offline','--cache',cache,...npmFlags],client,npmEnv,path.join(evidence,'sdk-offline-verification.log'));
checkClientFixtures();
const afterClient=files(client,new Set(['node_modules','generated']));
if(Object.keys(afterClient).length!==Object.keys(clientSource).length||Object.entries(clientSource).some(([p,h])=>afterClient[p]!==h))throw new Error('Client source closure changed during verification');
const generated=files(path.join(client,'generated'));
const clientVerification=JSON.parse(fs.readFileSync(path.join(client,'generated/verification.json'),'utf8'));
const compileInfo={rustc:run('rustc',['-vV'],source,env),node:run('node',['--version'],client,npmEnv).trim(),npm:run('npm',['--version',...npmFlags],client,npmEnv).trim(),host_tool_probes:{cc:run('cc',['--version'],source,env),ar:run('ar',['--version'],source,env)},c_tool_override_names:Object.keys(process.env).filter(k=>/^(CC|AR|CFLAGS|CPPFLAGS|CXX|CXXFLAGS)(_|$)/.test(k)).sort()};
authoritativeSources();check(vendor,engine.dependency_files);check(source,sourceBefore);serverPins();predecessorPins();
const hostTools=path.join(evidence,'host-tools');fs.mkdirSync(hostTools);const hostToolFiles:Record<string,string>={};for(const name of ['retain-server-evidence.ts','retain-server-evidence.test.ts','capture-json-output.ts','retain-policy-sources.ts']){const bytes=fs.readFileSync(path.join(root,'scripts',name));fs.writeFileSync(path.join(hostTools,name),bytes);hostToolFiles[name]=sha(bytes);}
const report={host_tool_files:hostToolFiles,status:'offline_server_and_client_verified',engine_build_id:engine.build_id,native_bundle_sha256:engine.bundle_sha256,server_executable:binary,server_executable_sha256:binarySha,server_schema_sha256:sha(JSON.stringify(schema)),server_native_info:nativeInfo,reference_server_executable_sha256:referenceSha,source_files:sourceBefore,cargo_vendor_manifest_sha256:engine.dependency_manifest_sha256,packages,empty_server_cargo_home_before_build:true,cargo_frozen_offline:true,client_source:clientSource,client_layout:{version:clientLayoutVersion,sdk_relative_root:'packages/game-server/sdk',native_fixture_files:clientNativeFixtureFiles},npm_preparation_cache_files:cacheManifest,empty_client_node_modules_before_install:true,npm_ci_offline:true,npm_ignore_scripts:true,client_generated:generated,client_verification:clientVerification,compile_info:compileInfo};
output(path.join(evidence,'evidence.json'),report);output(path.join(root,'generated/server-retention.json'),{evidence:path.join(evidence,'evidence.json'),sha256:sha(fs.readFileSync(path.join(evidence,'evidence.json'))),engine_build_id:engine.build_id});
console.log(JSON.stringify({status:report.status,evidence,engine_build_id:engine.build_id,server_executable_sha256:report.server_executable_sha256}));
