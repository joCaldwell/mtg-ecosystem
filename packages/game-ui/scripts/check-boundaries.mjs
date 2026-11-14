import { readFile, readdir } from 'node:fs/promises';
import { resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

/** Presentation consumes display values and intent slots. It owns no effects,
 * application data, gameplay protocol, router, authentication or fixtures. */
export function presentationViolations(file, source) {
  const errors = [];
  const text = source.replace(/\/\*[\s\S]*?\*\//g, '').replace(/^\s*\/\/.*$/gm, '');
  if (file.endsWith('.css')) {
    if (/(^|[},])\s*(?:html|body|:root)(?=[\s.{:#>])/m.test(text)) errors.push('Unscoped document selector');
    for (const variable of text.matchAll(/(?:var\()?(--[a-z0-9-]+)/g)) {
      if (!variable[1].startsWith('--game-')) errors.push('Presentation variables must be namespaced');
    }
    return [...new Set(errors)];
  }
  for (const match of text.matchAll(/\b(?:import|export)\s+(?:type\s+)?(?:[^;'"\n]*?\bfrom\s*)?['"]([^'"\n]+)['"]/g)) {
    const target = match[1];
    if (target === 'react') continue;
    if (!target.startsWith('./') || target.includes('..')) errors.push(`Private/external dependency: ${target}`);
  }
  if (/\b(?:fetch|XMLHttpRequest|WebSocket|EventSource|indexedDB|localStorage|sessionStorage|caches|Worker|SharedWorker|BroadcastChannel|navigator|globalThis|window|document)\b/.test(text)) errors.push('Application/browser effect');
  if (/\b(?:require|import)\s*\(/.test(text)) errors.push('Dynamic dependency');
  return [...new Set(errors)];
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const root = fileURLToPath(new URL('../src/', import.meta.url));
  const files = (await readdir(root, { recursive: true })).filter(file => /\.(?:tsx?|css)$/.test(file));
  const failures = [];
  for (const file of files) {
    for (const issue of presentationViolations(file, await readFile(resolve(root, file), 'utf8'))) failures.push(`${file}: ${issue}`);
  }
  if (failures.length) throw new Error(failures.join('\n'));
  console.log(`Pure shared presentation boundary passed (${files.length} files).`);
}
