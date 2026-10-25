import * as sqlite from "node:sqlite";

// Node 26.11 renamed the class (DEP0210). Prefer that name, while honoring
// the package's >=24.12 runtime floor on the Node 24 LTS line.
// https://nodejs.org/api/sqlite.html#class-database
export const Database = sqlite.Database ?? sqlite.DatabaseSync;
export type Database = sqlite.Database;
export { backup } from "node:sqlite";
