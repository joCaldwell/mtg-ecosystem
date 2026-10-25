// Offline, disposable schema input. Never open a real database for the CLI.
// From this package: npx auth@1.7.13 generate --config scripts/auth-schema.ts \
//   --output <temporary.sql> --yes
import { Database } from "../src/sqlite.ts";
import { configureAuth, authConfiguration } from "../src/auth-config.ts";
export const auth = configureAuth(authConfiguration(new Database(":memory:"), "http://localhost:8787", {
  BETTER_AUTH_SECRET: "offline-schema-placeholder-not-a-runtime-secret-0123456789",
}));
