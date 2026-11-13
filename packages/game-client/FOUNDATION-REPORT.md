# Client foundation evidence

Implementation base: `691496f6410c188d0e26f3a6f46d35f5ee159243` (reviewed plans
already merged). Owned paths: `packages/game-client/` only. No shared docs/root
package changes, Cargo invocation, hosted-profile change, deployment or merge.

## Initial shell checkpoint

Pinned Node 26.11.1; React/React DOM 19.3.1, React Router 8.5.1, Vite 8.3.8,
TypeScript 6.0.3, Playwright 1.65.0. Package-local install with exact lockfile and
`--workspaces=false --ignore-scripts`; root node_modules was absent and untouched.

Implemented strict TS structure, import/effect guards, table and card/decision
inspectors, declarative routes, unknown-data fallbacks and explicit disconnected
Join. Pure helpers cover draft token/seat/match invalidation, candidate selection,
ordering, incarnation identity, exact quantity syntax, generation fencing and
lifecycle-only projection retention. Journal/store interfaces are specifications,
not implemented IndexedDB or a connected session controller.

Initial focused checks: nine Node tests, strict TypeScript, import/effect boundaries,
production build, six browser cases (three each Chromium/Firefox). Browser cases
exercise keyboard navigation, decision jump, route refresh, 640px reduced-motion
layout, no API requests and no browser persistence. Production fixture preview
uses CSP without unsafe-eval/unsafe-inline and with connect-src none. Screenshots
were inspected at desktop and narrow sizes. This is automated inspection evidence,
not an actual human play session, a performance measurement or SDK validation proof.

Meaningful early findings: TypeScript 7.0.2's package no longer exposes the legacy
AST compiler API; initial typecheck failed, so the compatible stable 6.0.3 line was
pinned. Exact-optional checking caught an optional CSS class prop mismatch; fixed
without weakening strictness. Source review added a final-newline quantity case
because JavaScript `$` alone does not enforce canonical decimal termination.
Vite emits React Router's `use client` directive warning for this client-only
bundle; production browser execution is checked. Playwright reports this Linux
distribution is not officially supported and uses its Ubuntu fallback binaries.

## Still gated

The initial shell contains no invented card fixture. Native lane owns the exact
permitted Worker observation and the offline source-text producer. SDK lane owns
generated types/browser proof. Real data adaptation will be recorded separately
with exact source/artifact digests. No gameplay wire types have been handwritten.

No live authentication, Web Locks, resume/stream integration, persistent journal,
submission, available-action query, privacy-paired real-host test, dense board or
natural game acceptance ran. No M0-wide/M1/M2/M3/M4 acceptance is claimed. Native
action beginnings, decision semantic context and compiler source-ability labels
remain later producer gates; the client will not substitute rule heuristics.
