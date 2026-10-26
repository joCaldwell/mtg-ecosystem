// Enforces the layering table in web/ARCHITECTURE.md. Imports are read with a
// regex rather than a parser: the codebase uses plain import/export syntax,
// and a regex keeps this test free of dependencies.

import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const PACKAGE = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const SRC = join(PACKAGE, "web", "src");
const SERVER_SRC = join(PACKAGE, "src");

// ---------- reading the code ----------

interface Import {
  file: string; // relative to web/src, forward slashes
  spec: string;
  typeOnly: boolean;
  line: number;
}

function walk(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const full = join(dir, name);
    if (statSync(full).isDirectory()) walk(full, out);
    else out.push(full);
  }
  return out;
}

const posix = (p: string) => p.split(sep).join("/");

// Comments can hold import-looking text; blank them out but keep newlines so
// line numbers survive. Only whole-line `//` comments are removed: a `//`
// inside a string ("http://…") must not eat the rest of its line.
function stripComments(source: string): string {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, " "))
    .replace(/^[ \t]*\/\/.*$/gm, "");
}

// Static `import … from "x"`, `import "x"`, `export … from "x"` (with or
// without `type`), and dynamic `import("x")`.
const STATIC = /\b(import|export)\s+(type\s+)?(?:[^;'"]*?\bfrom\s*)?(["'])([^"'\n]+)\3/g;
const DYNAMIC = /\bimport\s*\(\s*(["'])([^"'\n]+)\1\s*\)/g;

export function extractImports(source: string, file = "<test>"): Import[] {
  const text = stripComments(source);
  const lineOf = (index: number) => text.slice(0, index).split("\n").length;
  const found: Import[] = [];
  for (const m of text.matchAll(STATIC)) {
    // `export const x = "…"` has no `from`, so it only matches if a quote
    // follows `export` directly, which is not valid syntax. `import type` and
    // `export type` set the flag; `import { type A }` does not, because Node
    // keeps that import at runtime.
    found.push({ file, spec: m[4], typeOnly: !!m[2], line: lineOf(m.index!) });
  }
  for (const m of text.matchAll(DYNAMIC)) found.push({ file, spec: m[2], typeOnly: false, line: lineOf(m.index!) });
  return found;
}

const files = walk(SRC).filter((f) => /\.(ts|tsx)$/.test(f));
const imports: Import[] = files.flatMap((f) =>
  extractImports(readFileSync(f, "utf8"), posix(relative(SRC, f))),
);

// ---------- placing files and targets in layers ----------

type Layer = "design" | "ui" | "mtg" | "domain" | "api" | "app" | "features" | "root" | "other";
const LAYERS = new Set(["design", "ui", "mtg", "domain", "api", "app", "features"]);

function layerOf(pathFromSrc: string): Layer {
  const first = pathFromSrc.split("/")[0];
  if (LAYERS.has(first)) return first as Layer;
  return pathFromSrc.includes("/") ? "other" : "root"; // main.tsx, vite-env.d.ts
}

interface Resolved {
  /** Path relative to web/src when the target is inside it, else null. */
  inSrc: string | null;
  /** True for the server's source tree (web/../src). */
  server: boolean;
  bare: boolean;
}

function resolveImport(imp: Import): Resolved {
  if (!imp.spec.startsWith(".")) return { inSrc: null, server: false, bare: true };
  const target = resolve(SRC, dirname(imp.file), imp.spec);
  const rel = posix(relative(SRC, target));
  if (!rel.startsWith("..")) return { inSrc: rel, server: false, bare: false };
  const fromServer = posix(relative(SERVER_SRC, target));
  return { inSrc: null, server: !fromServer.startsWith(".."), bare: false };
}

const describeImport = (imp: Import) => `${imp.file}:${imp.line} imports "${imp.spec}"`;

/** Collect messages for every import of `layer` that `violates`, and fail with all of them. */
function check(layer: Layer, violates: (imp: Import, to: Resolved, toLayer: Layer | null) => string | null) {
  const problems: string[] = [];
  for (const imp of imports) {
    if (layerOf(imp.file) !== layer) continue;
    const to = resolveImport(imp);
    const toLayer = to.inSrc !== null ? layerOf(to.inSrc) : null;
    const why = violates(imp, to, toLayer);
    if (why) problems.push(`${describeImport(imp)}: ${why}`);
  }
  assert.deepEqual(problems, [], `\n${problems.join("\n")}\n`);
}

// ---------- the extractor itself ----------

describe("import extraction", () => {
  const specs = (src: string) => extractImports(src).map((i) => [i.spec, i.typeOnly]);

  test("finds every import form", () => {
    const src = [
      `import a from "./a.ts";`,
      `import { b,\n  c } from '../b.ts';`,
      `import type { T } from "../api/client.ts";`,
      `import { type U } from "../api/client.ts";`,
      `import "./side.css";`,
      `export * from "./re.ts";`,
      `export type { V } from "./v.ts";`,
      `const m = await import("./lazy.ts");`,
    ].join("\n");
    assert.deepEqual(specs(src), [
      ["./a.ts", false],
      ["../b.ts", false],
      ["../api/client.ts", true],
      ["../api/client.ts", false],
      ["./side.css", false],
      ["./re.ts", false],
      ["./v.ts", true],
      ["./lazy.ts", false],
    ]);
  });

  test("ignores comments and plain exports", () => {
    const src = [
      `// import x from "./nope.ts";`,
      `/* import y from "./nope2.ts"; */`,
      `export const label = "from ./nope3.ts";`,
      `const url = "http://example.com"; import z from "./yes.ts";`,
    ].join("\n");
    assert.deepEqual(specs(src), [["./yes.ts", false]]);
  });

  test("reports line numbers", () => {
    assert.equal(extractImports(`\n\nimport a from "./a.ts";`)[0].line, 3);
  });

  test("the walker sees the files that exist", () => {
    assert.ok(files.length > 0, "no .ts/.tsx files found under web/src");
    assert.ok(files.some((f) => f.endsWith(join("api", "client.ts"))));
  });
});

// ---------- the table ----------

describe("layering", () => {
  test("ui imports only from ui (and design CSS)", () => {
    check("ui", (imp, to, toLayer) => {
      if (to.bare) return null; // react etc.
      if (to.server) return "ui must not reach the server source";
      if (toLayer === "ui") return null;
      if (toLayer === "design" && /\.css$/.test(imp.spec)) return null;
      return `ui may only import ui and design CSS, not ${toLayer ?? "outside web/src"}`;
    });
  });

  test("mtg imports only from mtg, ui and domain", () => {
    check("mtg", (_imp, to, toLayer) => {
      if (to.bare) return null;
      if (to.server) return "mtg must not reach the server source";
      if (toLayer === "mtg" || toLayer === "ui" || toLayer === "domain") return null;
      return `mtg may only import mtg, ui and domain, not ${toLayer ?? "outside web/src"}`;
    });
  });

  test("domain imports only from domain, and from api as import type", () => {
    check("domain", (imp, to, toLayer) => {
      if (to.bare) return "domain imports no packages (it has no React and no DOM)";
      if (to.server) return "domain reaches wire types through api/client.ts, not the server source";
      if (toLayer === "domain") return null;
      if (toLayer === "api") return imp.typeOnly ? null : "api may only be imported with `import type`";
      return `domain may only import domain and, type-only, api — not ${toLayer ?? "outside web/src"}`;
    });
  });

  test("domain never imports react", () => {
    check("domain", (imp) => (/^react(\/|-dom|$)/.test(imp.spec) ? "domain must not import react" : null));
  });

  test("api imports only from api, and from the server source as import type", () => {
    check("api", (imp, to, toLayer) => {
      if (to.bare) return null;
      if (to.server) return imp.typeOnly ? null : "server source may only be imported with `import type`";
      if (toLayer === "api") return null;
      return `api may only import api and, type-only, the server source — not ${toLayer ?? "outside web/src"}`;
    });
  });

  test("a feature reaches another feature only through its index.ts", () => {
    check("features", (imp, to) => {
      if (to.inSrc === null) return null;
      const [layer, target] = to.inSrc.split("/");
      const mine = imp.file.split("/")[1];
      if (layer !== "features" || !target || target === mine || /\.[a-z]+$/.test(target)) return null;
      const rest = to.inSrc.split("/").slice(2).join("/");
      return /^(index(\.tsx?)?)?$/.test(rest)
        ? null
        : `features/${mine} must import features/${target} only through its index.ts, not "${rest}"`;
    });
  });

  test("features never import main.tsx", () => {
    check("features", (_imp, to) =>
      to.inSrc !== null && /^main(\.tsx?)?$/.test(to.inSrc) ? "features must not import the entry point" : null,
    );
  });

  test("ui, mtg and domain never import features or app", () => {
    for (const layer of ["ui", "mtg", "domain"] as const)
      check(layer, (_imp, _to, toLayer) =>
        toLayer === "features" || toLayer === "app" ? `${layer} must not import ${toLayer}` : null,
      );
  });
});

// ---------- colours live in design/ ----------

describe("tokens", () => {
  test("CSS modules outside design/ contain no hex colour literals", () => {
    const HEX = /(?<!url\()#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})\b/g;
    const problems: string[] = [];
    for (const f of walk(SRC)) {
      const rel = posix(relative(SRC, f));
      if (!rel.endsWith(".module.css") || layerOf(rel) === "design") continue;
      const css = readFileSync(f, "utf8").replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, " "));
      css.split("\n").forEach((text, i) => {
        for (const m of text.matchAll(HEX))
          problems.push(`${rel}:${i + 1} has the colour ${m[0]}; use a token from design/tokens.css`);
      });
    }
    assert.deepEqual(problems, [], `\n${problems.join("\n")}\n`);
  });
});

// ---------- approved phone and tablet breakpoints (spec §11.2–11.3) ----------

describe("breakpoints", () => {
  test("width queries use exactly the phone, drawer and panel breakpoints", async () => {
    const { PHONE_MAX_WIDTH, DRAWER_BREAKPOINT, PANEL_BREAKPOINT } = await import("../web/src/app/breakpoints.ts");
    assert.deepEqual([PHONE_MAX_WIDTH, DRAWER_BREAKPOINT, PANEL_BREAKPOINT], [719, 960, 1100]);
    const problems: string[] = [];
    for (const f of walk(SRC)) {
      const rel = posix(relative(SRC, f));
      if (!rel.endsWith(".css")) continue;
      readFileSync(f, "utf8").split("\n").forEach((text, i) => {
        for (const m of text.matchAll(/@media[^{]*\((max|min)-width:\s*(\d+)px\)/g))
          if ((m[1] !== "max" || ![PHONE_MAX_WIDTH, DRAWER_BREAKPOINT - 1, PANEL_BREAKPOINT - 1].includes(Number(m[2]))) && !rel.startsWith("features/playtest/"))
            problems.push(`${rel}:${i + 1} uses ${m[1]} ${m[2]}px; allowed max widths are ${PHONE_MAX_WIDTH}, ${DRAWER_BREAKPOINT - 1}, ${PANEL_BREAKPOINT - 1} (app/breakpoints.ts)`);
      });
    }
    assert.deepEqual(problems, [], `\n${problems.join("\n")}\n`);
  });
});

// ---------- window scrolling (spec §11.2, §18.5) ----------

describe("window scrolling", () => {
  // base.css pins the body for the desktop panes; a pinned body cannot be
  // scrolled by touch or wheel, so a shell that scrolls the window must
  // release it. (Programmatic scrollTo still works, which is how this hid.)
  test("the phone and shared-page shells release the pinned body", () => {
    const base = readFileSync(join(SRC, "design", "base.css"), "utf8");
    assert.match(base, /body\s*\{[^}]*overflow:\s*hidden/);
    assert.match(base, /html\[data-scroll="window"\] body\s*\{[^}]*overflow:\s*visible/);
    for (const shell of ["features/phone/PhoneShell.tsx", "features/public/PublicShell.tsx", "app/TabletShell.tsx"])
      assert.match(readFileSync(join(SRC, shell), "utf8"), /^\s*useWindowScroll\(\);/m, `${shell} must call useWindowScroll()`);
    // AppShell keeps <main> in place and mounts the tablet pieces around it.
    assert.match(readFileSync(join(SRC, "app/AppShell.tsx"), "utf8"), /\{tablet && <TabletScroll \/>\}/, "the tablet frame must mount TabletScroll");
  });

  // A review runs in the list on tablets: a modal sheet over it would hide it
  // and switch off its keys (useHotkeys yields to an open modal).
  test("starting a review does not open the tablet sheet", () => {
    const body = stripComments(readFileSync(join(SRC, "features/deck/DeckBody.tsx"), "utf8"));
    assert.doesNotMatch(body, /ui\.reviewing/);
  });
});

// Components use the typed client, including examples on /design.
test("fetch stays in the API layer", () => {
  const problems = files.filter(f => /\.tsx$/.test(f) && /\bfetch\s*\(/.test(stripComments(readFileSync(f, "utf8")))).map(f => posix(relative(SRC, f)));
  assert.deepEqual(problems, []);
});

// vite.config.ts may skip unused re-exports without running editor modules.
test("tree-shaken public barrels contain only re-exports", () => {
  for (const f of files.filter(f => /(?:features\/[^/]+|mtg|ui)\/index\.ts$/.test(posix(f)))) {
    const rest = stripComments(readFileSync(f, "utf8")).replace(/export\s+(?:type\s+)?(?:\{[^}]*\}|\*)\s+from\s+["'][^"']+["'];?/g, "").trim();
    assert.equal(rest, "", `${relative(SRC, f)} must not have module side effects`);
  }
});
