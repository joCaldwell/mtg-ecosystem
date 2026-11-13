import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { build } from 'esbuild';
import ts from 'typescript';

const sdk = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const generated = path.join(sdk, 'generated');
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const generation = JSON.parse(fs.readFileSync(path.join(generated, 'generation.json')));
for (const file of [...generation.files, { path: 'protocol.schema.json', sha256: generation.schema_sha256 }]) {
  if (sha(fs.readFileSync(path.join(generated, file.path))) !== file.sha256) throw new Error(`Stale generated artifact: ${file.path}`);
}
for (const file of generation.standalone.sources) {
  if (sha(fs.readFileSync(path.join(sdk, file.path))) !== file.sha256) throw new Error(`Regenerate after source change: ${file.path}`);
}
const pending = fs.mkdtempSync(path.join(generated, '.browser-'));
try {
  const result = await build({
    absWorkingDir: sdk, entryPoints: ['src/browser.ts'], outfile: path.join(pending, 'browser.js'),
    bundle: true, minify: true, platform: 'browser', format: 'esm', target: 'es2023',
    metafile: true, legalComments: 'inline',
  });
  const allowed = new Set(['src/browser.ts', 'src/client.ts', 'src/validation.ts', 'src/strict-json.ts',
    'src/public.ts', 'src/transport.ts', 'generated/presentation-identity.ts', 'generated/validators.js', 'generated/routes.ts', 'generated/protocol.ts']);
  for (const input of Object.keys(result.metafile.inputs)) {
    if (!allowed.has(input)) throw new Error(`Unexpected browser runtime dependency: ${input}`);
  }
  const outputs = Object.values(result.metafile.outputs);
  if (outputs.some(output => output.imports.length)) throw new Error('Browser bundle has external runtime dependencies');
  const config = ts.readConfigFile(path.join(sdk, 'tsconfig.json'), ts.sys.readFile);
  if (config.error) throw new Error(ts.flattenDiagnosticMessageText(config.error.messageText, '\n'));
  const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, sdk);
  const program = ts.createProgram([path.join(sdk, 'src/browser.ts')], { ...parsed.options,
    noEmit: false, declaration: true, emitDeclarationOnly: true,
    rootDir: sdk, outDir: path.join(pending, 'types'),
  });
  const diagnostics = ts.getPreEmitDiagnostics(program);
  if (diagnostics.length) throw new Error(ts.formatDiagnosticsWithColorAndContext(diagnostics, {
    getCurrentDirectory: () => sdk, getCanonicalFileName: name => name, getNewLine: () => '\n',
  }));
  const emitted = program.emit(undefined, (filename, content) => {
    fs.mkdirSync(path.dirname(filename), { recursive: true });
    fs.writeFileSync(filename, content.replace(/(from\s+['"]\.[^'"]+)\.ts(['"])/g, '$1.js$2'));
  });
  if (emitted.emitSkipped) throw new Error('Browser declarations were not emitted');
  for (const file of generation.files.filter(file => file.path.endsWith('.d.ts'))) {
    fs.copyFileSync(path.join(generated, file.path), path.join(pending, 'types/generated', file.path));
  }
  fs.writeFileSync(path.join(pending, 'browser.d.ts'), "export * from './types/src/browser.js';\n");
  const inventory = directory => fs.readdirSync(directory, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name)).flatMap(entry => {
    const file = path.join(directory, entry.name);
    return entry.isDirectory() ? inventory(file) : [{ path: path.relative(pending, file), sha256: sha(fs.readFileSync(file)), bytes: fs.statSync(file).size }];
  });
  const sourcePaths = [...Object.keys(result.metafile.inputs), 'scripts/build-browser.mjs', 'package.json', 'package-lock.json', 'tsconfig.json'];
  const manifest = { format_version: 1, schema_sha256: generation.schema_sha256,
    generation_sha256: sha(fs.readFileSync(path.join(generated, 'generation.json'))),
    node: process.version, esbuild: '0.28.2', typescript: ts.version,
    sources: sourcePaths.sort().map(file => ({ path: file, sha256: sha(fs.readFileSync(path.join(sdk, file))) })),
    runtime_inputs: Object.keys(result.metafile.inputs).sort(), runtime_exports: outputs[0].exports.sort(),
    files: inventory(pending),
  };
  fs.writeFileSync(path.join(pending, 'build.json'), JSON.stringify(manifest, null, 2) + '\n');
  // Replace only this build's disposable SDK dist, after all checks succeed.
  fs.mkdirSync(path.join(sdk, 'dist'), { recursive: true });
  for (const entry of fs.readdirSync(pending)) fs.cpSync(path.join(pending, entry), path.join(sdk, 'dist', entry), { recursive: true });
  console.log(JSON.stringify({ status: 'browser-built', schema_sha256: generation.schema_sha256,
    bytes: manifest.files.find(file => file.path === 'browser.js').bytes, runtime_exports: manifest.runtime_exports }));
} finally { fs.rmSync(pending, { recursive: true, force: true }); }
