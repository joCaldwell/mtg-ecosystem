// Fail closed when a renamed/empty test selection executes no real tests.
// Use per-file summaries: Node can count a test-file wrapper as a global pass.
// https://nodejs.org/api/test.html#event-testsummary
import { run } from "node:test";
import { spec } from "node:test/reporters";

const [flag, pattern, ...files] = process.argv.slice(2);
if (flag !== "--name" || !pattern || files.length === 0 || files.some(file => file.startsWith("-"))) {
  throw new Error("Usage: node scripts/run-selected-tests.ts --name REGEXP FILE...");
}
const expression = new RegExp(pattern);
const stream = run({ files, testNamePatterns: expression });
let executed = 0;
let success = false;
stream.on("test:summary", summary => {
  if (summary.file) {
    executed += summary.counts.tests - summary.counts.skipped - summary.counts.todo;
  } else {
    success = summary.success;
  }
});
for await (const output of stream.compose(spec)) process.stdout.write(output);
if (executed === 0) console.error(`No runnable tests matched ${JSON.stringify(pattern)} in ${files.join(", ")}. Check the selector or renamed test.`);
process.exitCode = success && executed > 0 ? 0 : 1;
