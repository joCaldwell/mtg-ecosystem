import { readFile, readdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { boundaryViolations } from './boundaries.ts';

const root = fileURLToPath(new URL('../src/', import.meta.url));
const files = (await readdir(root, { recursive: true })).filter(file => /\.tsx?$/.test(file));
const errors: string[] = [];
for (const file of files) {
  for (const issue of boundaryViolations(file, await readFile(`${root}/${file}`, 'utf8'))) errors.push(`${file}: ${issue}`);
}
if (errors.length) throw new Error(errors.join('\n'));
console.log(`Import/effect boundaries passed (${files.length} source files).`);
