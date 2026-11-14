import { readFile } from 'node:fs/promises';
import { resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';

const root = fileURLToPath(new URL('../src/', import.meta.url));
const seen = new Set<string>();
const allowedSource = /^(?:preview\/|presentation\/opening-fixture\.ts$|features\/decisions\/(?:DecisionPanel|LocalControls)\.tsx$|decisions\/)/;
const permittedFixtures = new Set(['../fixtures/foundations-opening/opening-seat0-http.json', '../fixtures/foundations-opening/presentation.json']);
async function visit(file: string): Promise<void> {
  if (seen.has(file)) return;
  seen.add(file);
  const path = relative(root, file).replaceAll('\\', '/');
  if (path.endsWith('.css')) return;
  if (path.endsWith('.json') && permittedFixtures.has(path)) return;
  if (!allowedSource.test(path)) throw new Error(`Preview reaches private application source: ${path}`);
  const source = await readFile(file, 'utf8');
  const ast = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true, file.endsWith('tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  for (const node of ast.statements) {
    if (!(ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) || !node.moduleSpecifier || !ts.isStringLiteral(node.moduleSpecifier)) continue;
    const typeOnly = ts.isImportDeclaration(node) ? node.importClause?.isTypeOnly : node.isTypeOnly;
    if (typeOnly) continue;
    const spec = node.moduleSpecifier.text;
    if (['react', '@multiverse/game-ui'].includes(spec)) continue;
    if (!spec.startsWith('.')) throw new Error(`Preview runtime dependency is not presentation: ${spec}`);
    await visit(resolve(file, '..', spec));
  }
  function effects(node: ts.Node): void {
    if (ts.isCallExpression(node) && (node.expression.kind === ts.SyntaxKind.ImportKeyword || (ts.isIdentifier(node.expression) && node.expression.text === 'require'))) throw new Error(`Preview dynamic dependency: ${path}`);
    if (ts.isIdentifier(node) && ['fetch', 'XMLHttpRequest', 'WebSocket', 'EventSource', 'indexedDB', 'localStorage', 'sessionStorage', 'caches', 'Worker', 'SharedWorker', 'BroadcastChannel'].includes(node.text)) throw new Error(`Preview effect: ${path}`);
    ts.forEachChild(node, effects);
  }
  effects(ast);
}
await visit(resolve(root, 'preview/index.ts'));
console.log(`Router-free public preview runtime boundary passed (${seen.size} modules).`);
