import { homedir } from "node:os";
import { join, resolve } from "node:path";

export const UX_EVAL_ROOT = resolve(process.env.UX_EVAL_ROOT ?? join(homedir(), ".cache", "deck-builder-ux-eval"));
