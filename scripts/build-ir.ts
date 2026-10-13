// scripts/build-ir.ts
// Full pipeline: ingest → parse → emit per-set Card IR.
//
// BULK PIPELINE NOT YET IMPLEMENTED. The Rust mtg-sim compile command lowers
// only the reviewed first-scenario fixtures. General executable emission needs
// semantic validation, binding, and whole-profile admission, not syntax coverage.

console.error("build-ir is not implemented yet.");
console.error("The first-scenario fixture compiler is available through cargo run --locked -p mtg-sim -- compile.");
console.error("Executable Card IR requires semantic validation, reference binding, and a supported engine contract; acceptance coverage alone is insufficient.");
console.error("Use `npm run archive-parser` to store complete source records and nonexecutable AST results, or `npm run validate` to measure parsing acceptance.");
process.exit(1);
