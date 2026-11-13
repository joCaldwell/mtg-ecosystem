/** Explicit public build exporter. No Cargo, dependency install, directory server,
 * private file discovery or recursive copying. Pass only reviewed dist assets. */
import { readFileSync, lstatSync, mkdirSync, writeFileSync } from 'node:fs';
import { resolve, dirname, basename } from 'node:path';
import { fileURLToPath } from 'node:url';
import { canonical, sha256 } from './client-artifacts.ts';
export function exportClient(dist: string, output: string, files: string[]) {
  dist = resolve(dist); output = resolve(output);
  if (files.length < 1 || files.length > 128 || new Set(files).size !== files.length || !files.includes('index.html')) throw new Error('Invalid public file list');
  let total = 0;
  const copies = files.map(path => {
    if (path !== 'index.html' && !/^assets\/[A-Za-z0-9_-][A-Za-z0-9_.-]*\.(js|css|woff2|png|webp|jpg|jpeg)$/.test(path)) throw new Error('Unsupported public path');
    if (path.includes('..') || basename(path).length > 160) throw new Error('Unsupported public path');
    const source = resolve(dist, path);
    for (let part = source;; part = dirname(part)) {
      if (lstatSync(part).isSymbolicLink()) throw new Error('Public input symlink');
      if (dirname(part) === part) break;
    }
    const stat = lstatSync(source);
    if (!stat.isFile() || stat.size > 16 * 1024 * 1024) throw new Error('Public input bound');
    const bytes = readFileSync(source); total += bytes.length;
    if (total > 64 * 1024 * 1024) throw new Error('Public bundle bound');
    return { path, bytes };
  });
  // Fail before creating output for invalid inputs. A partial IO failure is kept
  // for diagnosis; a subsequent export must choose a new output directory.
  mkdirSync(output); mkdirSync(resolve(output, 'assets'));
  const pins = copies.map(({path, bytes}) => {
    writeFileSync(resolve(output, path), bytes, { flag: 'wx', mode: 0o644 });
    return {path, sha256: sha256(bytes)};
  });
  const manifest = canonical({bundle_version:1, files:pins}) + '\n';
  writeFileSync(resolve(output, 'client-assets.json'), manifest, {flag:'wx', mode:0o644});
  return {bundle:output, bundle_sha256:sha256(manifest), files:pins.length, bytes:total};
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [dist, output, ...files] = process.argv.slice(2);
  if (!dist || !output) throw new Error('usage: export-client.ts DIST NEW_PUBLIC_BUNDLE index.html assets/EXACT.js assets/EXACT.css ...');
  console.log(JSON.stringify(exportClient(dist, output, files)));
}
