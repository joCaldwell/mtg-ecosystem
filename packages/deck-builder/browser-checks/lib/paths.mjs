import { homedir } from "node:os";
import { join, resolve } from "node:path";

export const CHECKS_ROOT = resolve(process.env.CHECKS_ROOT ?? join(homedir(), ".cache", "deck-builder-browser-checks"));
