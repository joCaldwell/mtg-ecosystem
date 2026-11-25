// This pass resolves reference identities, not game execution or all semantic
// correctness. In particular, it never substitutes "last noun" for an
// ambiguous referent. Every unsupported scope remains a reported obligation.
// Rules: CR 109.5, 201.5, 400.7j, 601.2c, 608.2h/k, 107.3, 607.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
// The binder is implemented in ./binder/; this module is its public entry point.
export { analyzeCardBindings } from "./binder/card.ts";
