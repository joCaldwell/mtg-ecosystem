# Shared game presentation

This package is the public display boundary used by the unified product and the
standalone game-client harness. Read the root manual before editing.

- Own `GameTable`, card inspection, card faces, generic facts and display-only
  models. Inputs must already be authorized observations or qualified fixtures.
- No gameplay wire definitions, SDK, session/controller, account, routing, I/O,
  compiler/IR, source fetching or application-private dependencies.
- Never compute legality or infer facts from printed card text. Unknown values
  remain explicit. Preserve exact handle selection and keyboard focus return.
- CSS Modules and `--game-*` tokens are scoped to `GameSurface`/component roots.
  No `:root`, body or document styles. React is a peer; hosts deduplicate it.
- Only public package exports may be consumed from outside. Fixtures belong to a
  separately qualified preview adapter, never this package.
- Run `npm --prefix packages/game-ui run check --workspaces=false` with pinned
  Node through `scripts/engine-env.sh`; integration browser checks belong to the
  consumer. Do not start Cargo or regenerate SDK artifacts.
