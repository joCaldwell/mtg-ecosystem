// scripts/build-ir.ts
// Full pipeline: ingest → parse → emit per-set Card IR.
//
// NOT YET IMPLEMENTED. Executable IR requires semantic validation and reference
// binding, not just syntactic coverage. `archive-parser` can persist source and
// structured parser results without pretending to perform semantic lowering.

console.error("build-ir is not implemented yet.");
console.error("Executable Card IR requires semantic validation, reference binding, and a supported engine contract; acceptance coverage alone is insufficient.");
console.error("Use `npm run archive-parser` to store complete source records and nonexecutable AST results, or `npm run validate` to measure parsing acceptance.");
process.exit(1);
