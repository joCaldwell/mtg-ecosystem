import ts from 'typescript';

const layers: Record<string, readonly string[]> = {
  app: ['app', 'features', 'session', 'presentation', 'design', 'ui'],
  features: ['features', 'cards', 'decisions', 'presentation', 'ui', 'session'],
  cards: ['cards', 'presentation', 'ui'],
  presentation: ['presentation'],
  decisions: ['decisions'],
  session: ['session', 'decisions', 'presentation'],
  ui: ['ui'],
};
const effects = new Set(['fetch', 'XMLHttpRequest', 'WebSocket', 'EventSource', 'indexedDB', 'localStorage', 'sessionStorage', 'caches', 'Worker', 'SharedWorker', 'BroadcastChannel']);
const browserGlobals = new Set(['window', 'document', 'navigator', 'globalThis', 'self']);

/** Source-level guard, complementary to review. It is not a semantic sandbox. */
export function boundaryViolations(file: string, source: string): string[] {
  const issues: string[] = [];
  const layer = file.split('/')[0] ?? '';
  const pure = ['presentation', 'decisions'].includes(layer) || (layer === 'session' && !file.startsWith('session/effects/'));
  const canEffect = file.startsWith('session/effects/');
  const parsed = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true, file.endsWith('tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  function checkImport(specifier: string, typeOnly: boolean) {
    if (typeOnly && ['app', 'presentation'].includes(layer) && specifier === '../../generated/presentation.d.ts') return;
    if (layer === 'app' && /^\.\.\/\.\.\/fixtures\/foundations-opening\/(opening-seat0-http|presentation)\.json$/.test(specifier)) return;
    if (specifier.startsWith('.')) {
      const path = file.split('/').slice(0, -1);
      for (const part of specifier.split('/')) {
        if (part === '..') {
          if (!path.length) { issues.push(`Import escapes client src: ${specifier}`); return; }
          path.pop();
        } else if (part !== '.') path.push(part);
      }
      const target = path[0] ?? '';
      if (!(layers[layer] ?? []).includes(target)) issues.push(`Forbidden layer import: ${specifier}`);
      return;
    }
    if (specifier.startsWith('@multiverse/game-server-sdk')) {
      if (!['@multiverse/game-server-sdk', '@multiverse/game-server-sdk/browser'].includes(specifier)) issues.push('Only public SDK entrypoints are permitted');
      if (!['app', 'session', 'presentation', 'decisions'].includes(layer) || (!typeOnly && !canEffect)) issues.push('SDK runtime belongs only in session/effects');
      return;
    }
    if (pure || !['react', 'react-dom/client', 'react-router'].includes(specifier)) issues.push(`Forbidden external import: ${specifier}`);
  }
  function visit(node: ts.Node): void {
    if (ts.isImportDeclaration(node) && ts.isStringLiteral(node.moduleSpecifier)) checkImport(node.moduleSpecifier.text, node.importClause?.isTypeOnly ?? false);
    if (ts.isExportDeclaration(node) && node.moduleSpecifier && ts.isStringLiteral(node.moduleSpecifier)) checkImport(node.moduleSpecifier.text, node.isTypeOnly);
    if (ts.isCallExpression(node) && (node.expression.kind === ts.SyntaxKind.ImportKeyword || (ts.isIdentifier(node.expression) && node.expression.text === 'require'))) {
      issues.push('Dynamic/require import requires explicit boundary review');
    }
    if (ts.isIdentifier(node) && ((effects.has(node.text) && !canEffect) || (pure && browserGlobals.has(node.text)))) issues.push(`Forbidden effect/global: ${node.text}`);
    if (ts.isElementAccessExpression(node) && ts.isStringLiteral(node.argumentExpression) && effects.has(node.argumentExpression.text) && !canEffect) issues.push(`Forbidden computed effect: ${node.argumentExpression.text}`);
    if (ts.isIdentifier(node) && ['eval', 'Function'].includes(node.text)) issues.push('Dynamic code execution forbidden');
    ts.forEachChild(node, visit);
  }
  visit(parsed);
  return [...new Set(issues)];
}
