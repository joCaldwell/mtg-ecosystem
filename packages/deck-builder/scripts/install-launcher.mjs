#!/usr/bin/env node

import { chmodSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const PKG_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const EXECUTABLE = join(PKG_ROOT, "deck-builder");
const DATA_HOME = process.env.XDG_DATA_HOME || join(homedir(), ".local", "share");
const APPLICATIONS_DIR = join(DATA_HOME, "applications");
const DESKTOP_FILE = join(APPLICATIONS_DIR, "mtg-deck-builder.desktop");

// The desktop-entry Exec field has its own quoting rules (it is not a shell).
const quoteExecArg = (value) => `"${value.replace(/["`$\\]/g, "\\$&")}"`;

if (process.argv.includes("--uninstall")) {
  rmSync(DESKTOP_FILE, { force: true });
  console.log(`Removed ${DESKTOP_FILE}`);
  process.exit(0);
}

mkdirSync(APPLICATIONS_DIR, { recursive: true });
chmodSync(EXECUTABLE, 0o755);
writeFileSync(
  DESKTOP_FILE,
  `[Desktop Entry]
Type=Application
Version=1.0
Name=MTG Deck Builder
Comment=Build and manage Commander decks locally
Exec=${quoteExecArg(EXECUTABLE)} --notify
Icon=applications-games
Terminal=false
Categories=Game;
StartupNotify=false
`,
  { mode: 0o755 },
);
chmodSync(DESKTOP_FILE, 0o755);

console.log(`Installed MTG Deck Builder in your app launcher.`);
console.log(`Desktop entry: ${DESKTOP_FILE}`);
