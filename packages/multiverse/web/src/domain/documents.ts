import type { DocumentEdit } from "../api/client.ts";
import { diffLines } from "./diff.ts";
import type { DiffLine } from "./diff.ts";

/** Profile, thesis and constraints are text the owner reads as a diff; engines are not. */
export function isTextEdit(edit: DocumentEdit): boolean {
  return edit.kind === "profile" || edit.kind === "thesis" || edit.kind === "constraints" || edit.kind === "power_target";
}

/**
 * What the edit changes. The base is the text as it was when the edit was
 * proposed, so the diff shows what the agent saw; `current` covers an edit
 * that carries no base.
 */
export function editDiff(edit: DocumentEdit): DiffLine[] {
  if (!isTextEdit(edit)) return [];
  return diffLines(edit.payload.base ?? edit.current, edit.payload.content ?? "");
}

export function editTitle(edit: DocumentEdit): string {
  const engine = `“${edit.payload.engine_name ?? "unnamed"}”`;
  switch (edit.kind) {
    case "power_target":
      return "Edit to the power target";
    case "profile":
      return "Edit to your profile";
    case "thesis":
      return "Edit to the thesis";
    case "constraints":
      return "Edit to the constraints";
    case "engine_set":
      return `Add or change the engine ${engine}`;
    case "engine_remove":
      return `Remove the engine ${engine}`;
  }
}
