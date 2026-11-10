import type { ImportDiff } from "../api/client.ts";

export function importName(name: string, diff: ImportDiff): string {
  return name.trim() || diff.adds.filter(card => card.role === "commander").map(card => card.name).join(" + ") || "Imported deck";
}
export function hasImportChanges(diff: ImportDiff): boolean {
  return diff.add_count + diff.cut_count + diff.role_changes.length + diff.set_aside.length > 0;
}
