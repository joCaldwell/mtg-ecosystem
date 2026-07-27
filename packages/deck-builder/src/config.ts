import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const PKG_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

// Load .env once at import time; absent file is fine (env vars may be set
// directly in the shell).
try {
  process.loadEnvFile(join(PKG_ROOT, ".env"));
} catch {
  /* no .env file */
}

export interface AgentConfig {
  apiKey: string;
  model: string;
  baseUrl: string;
}
// Decision-log retention N (spec §12) is a per-install setting, not agent
// config — see getRetentionN/setRetentionN in db.ts.

export function getAgentConfig(): AgentConfig {
  const apiKey = process.env.OPENROUTER_API_KEY ?? "";
  const model = process.env.OPENROUTER_MODEL ?? "";
  if (!apiKey)
    throw new ConfigError(
      "OPENROUTER_API_KEY is not set. Copy .env.example to .env in packages/deck-builder and add your key.",
    );
  if (!model)
    throw new ConfigError(
      "OPENROUTER_MODEL is not set. Add a model slug (e.g. openai/gpt-5.6-sol) to packages/deck-builder/.env.",
    );
  return {
    apiKey,
    model,
    baseUrl: process.env.OPENROUTER_BASE_URL ?? "https://openrouter.ai/api/v1",
  };
}

export class ConfigError extends Error {}
