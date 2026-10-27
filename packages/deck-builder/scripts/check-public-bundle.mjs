import fs from 'node:fs';
import { gzipSync } from 'node:zlib';
const root=new URL('../web/dist/',import.meta.url), manifest=JSON.parse(fs.readFileSync(new URL('.vite/manifest.json',root)));
const seen=new Set();function visit(key){if(seen.has(key))return;seen.add(key);for(const dep of manifest[key].imports??[])visit(dep);}
visit('index.html');visit('src/features/public/PublicDeckPage.tsx');
const chunks=[...seen].filter(k=>manifest[k].file.endsWith('.js')).map(k=>({file:manifest[k].file,gzip:gzipSync(fs.readFileSync(new URL(manifest[k].file,root))).length}));
const bytes=chunks.reduce((sum,c)=>sum+c.gzip,0); console.log(JSON.stringify({bytes,kB:bytes/1000,chunks},null,2));if(bytes>=110000)process.exitCode=1;
