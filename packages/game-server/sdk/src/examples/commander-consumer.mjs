// Commander-development private bootstrap/output adapter; the strict TypeScript policy and
// loop use GameClient exclusively. No credentials in argv or environment.
import { writeFile } from 'node:fs/promises';
import { parseBootstrap, runFoundationsConsumer } from './foundations-consumer.ts';
import { commanderDevelopmentPolicy } from './commander-development-policy.ts';
import { strictJSON } from '../strict-json.ts';

const started = performance.now();
let bootstrap;
let report;
const stopped = reason => ({ consumer_version: 1, status: 'stopped', reason });
try {
  if (process.argv.length !== 2) throw new Error('private bootstrap required');
  let size = 0;
  const chunks = [];
  const timer = setTimeout(() => process.stdin.destroy(new Error('bootstrap timeout')), 5_000);
  try {
    for await (const chunk of process.stdin) {
      size += chunk.length;
      if (size > 16_384) throw new Error('bootstrap bound');
      chunks.push(chunk);
    }
  } finally { clearTimeout(timer); }
  const text = new TextDecoder('utf-8', { fatal: true }).decode(Buffer.concat(chunks));
  bootstrap = parseBootstrap(strictJSON(text));
} catch { report = stopped('bootstrap_invalid'); }

if (bootstrap !== undefined) {
  const controller = new AbortController();
  const cancel = () => controller.abort();
  process.once('SIGINT', cancel);
  process.once('SIGTERM', cancel);
  try {
    report = await runFoundationsConsumer(bootstrap, {
      signal: controller.signal,
      policy: commanderDevelopmentPolicy,
      limits: { durationMs: Math.max(1, 25 * 60_000 - Math.ceil(performance.now() - started)) },
    });
  }
  catch { report = stopped('consumer_failure'); }
  finally { process.removeListener('SIGINT', cancel); process.removeListener('SIGTERM', cancel); }
}
// Defense in depth: even malicious server metadata may not echo this bearer.
if (bootstrap !== undefined && JSON.stringify(report).includes(bootstrap.capability)) report = stopped('private_output_rejected');
if (bootstrap?.outputPath !== undefined) {
  try { await writeFile(bootstrap.outputPath, JSON.stringify(report) + '\n', { flag: 'wx', mode: 0o600 }); }
  catch { report = stopped('report_output_failed'); }
}
process.stdout.write(JSON.stringify(report) + '\n');
process.exitCode = report?.status === 'completed' ? 0 : 2;
