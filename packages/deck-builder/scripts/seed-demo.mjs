#!/usr/bin/env node
// CLI adapter; the same capability serves first-run onboarding.
import { openDb } from "../src/db.ts";
import { localActor } from "../src/mode.ts";
import { createDemo } from "../src/demo.ts";
const db = openDb();
try {
  const result = createDemo(db, localActor());
  console.log(`Sample deck #${result.deckId}; ${result.missing.length} unavailable cards skipped.`);
} finally { db.close(); }
