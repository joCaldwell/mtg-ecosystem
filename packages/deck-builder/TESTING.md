# Backend and product test gates

Since the unified product extraction, run separate gates from the repository root:

```sh
sh scripts/engine-env.sh npm --prefix packages/deck-builder run check --workspaces=false
sh scripts/engine-env.sh npm --prefix packages/multiverse run check --workspaces=false
sh scripts/engine-env.sh npm --prefix packages/multiverse run build:web --workspaces=false
```

The backend owns capability/auth/tenant, storage, host, API and MCP tests. Product
route, inbox-display and frontend architecture tests now live in
`packages/multiverse/test/`. Shared display and standalone session/decision changes
also use their own game-ui/game-client checks. Disposable integrated browser
regressions remain in [browser-checks](browser-checks/README.md).

## Coverage and omissions

Keep capability permissions and tenant isolation, public allowlist privacy,
migration/content preservation, backup/restore, exact card resolution/suggestions,
membership/undo, proposals, representative import formats, hosted sessions/headers
and a real MCP client smoke. Product route/architecture checks live in Multiverse;
UI copy/markup/CSS snapshots and duplicate transport matrices are not a substitute
for behavior checks. A focused gate is representative, not exhaustive security coverage.

Plain-text Moxfield exports and the Najeela printing fixture remain inputs to
browser checks. Use disposable actor-scoped data, never production owner records.
[Product/client evidence](../../docs/game-client-evidence.md) records accepted
source and omissions; historical suite-reduction counts/deletion lists are in Git,
not current gate instructions. Preserve meaningful failed results.
