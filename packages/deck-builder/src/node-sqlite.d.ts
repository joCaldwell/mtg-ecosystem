// Node 26.11 renamed the class (DEP0210); @types/node still uses its old name.
// Type-only forward declaration, removable when DefinitelyTyped catches up.
import "node:sqlite";
declare module "node:sqlite" {
  export { DatabaseSync as Database };
}
