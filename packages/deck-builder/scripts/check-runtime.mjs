// Generate a TS-AST import manifest in the build stage; check it without
// TypeScript in the final image. Resolving never executes server/CLI code.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, existsSync, globSync } from 'node:fs';
import { dirname, resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
const file = process.argv[3] ? resolve(process.argv[3]) : join(root, 'runtime-imports.json');
if (process.argv[2] === '--manifest') {
  const { default: ts } = await import('typescript');
  const imports = [];
  for (const path of globSync('src/**/*.ts', { cwd: root })) {
    const ast = ts.createSourceFile(path, readFileSync(join(root, path), 'utf8'), ts.ScriptTarget.Latest, true);
    const visit = node => {
      if ((ts.isImportDeclaration(node) && !node.importClause?.isTypeOnly || ts.isExportDeclaration(node) && !node.isTypeOnly) && node.moduleSpecifier && ts.isStringLiteral(node.moduleSpecifier))
        imports.push({ file: path, name: node.moduleSpecifier.text });
      if (ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword && node.arguments[0] && ts.isStringLiteral(node.arguments[0]))
        imports.push({ file: path, name: node.arguments[0].text });
      ts.forEachChild(node, visit);
    };
    visit(ast);
  }
  writeFileSync(file, JSON.stringify(imports));
} else {
  const imports = JSON.parse(readFileSync(file, 'utf8'));
  const external = new Set(Object.keys(pkg.dependencies));
  for (const { file: source, name } of imports) {
    if (name.startsWith('.')) assert.ok(existsSync(resolve(root, dirname(source), name)), `${source}: missing ${name}`);
    else if (!name.startsWith('node:')) external.add(name);
  }
  const versions = {};
  for (const name of [...external].sort()) {
    let path = dirname(fileURLToPath(import.meta.resolve(name)));
    const packageName = name.startsWith('@') ? name.split('/').slice(0, 2).join('/') : name.split('/')[0];
    for (;;) {
      const file = join(path, 'package.json');
      if (existsSync(file)) {
        const dependency = JSON.parse(readFileSync(file, 'utf8'));
        if (dependency.name === packageName) { versions[packageName] = dependency.version; break; }
      }
      const parent = dirname(path); assert.notEqual(parent, path, `Missing package metadata for ${name}`); path = parent;
    }
  }
  for (const [name, range] of Object.entries(pkg.dependencies))
    if (/^\d+\.\d+\.\d+$/.test(range)) assert.equal(versions[name], range, `${name} resolved to a different version`);
  assert.equal(versions.jose, '6.2.3');
  console.log(JSON.stringify({ runtime_imports_checked: imports.length, versions }));
}
