import { bucket, defineRailway, github, preserve, project, service, volume } from "railway/iac";

// CLI 5.63.1 / SDK 3.16.0; evaluate for each linked environment.
// https://docs.railway.com/infrastructure-as-code/reference
export default defineRailway(ctx => {
  const environment = ctx.environmentName ?? ctx.environment;
  if (ctx.projectName !== "deck-builder" || !["production", "staging"].includes(environment ?? ""))
    throw new Error("Link deck-builder and production or staging before planning");
  const production = environment === "production";
  const data = volume(`${environment}-data`, { sizeMB: 5000, region: "ams" });
  const backups = bucket(`backups-${environment}`, { region: "ams" });
  const app = service("deck-builder", {
    source: github("joCaldwell/mtg-ecosystem", { branch: production ? "release" : "main" }),
    build: {
      builder: "DOCKERFILE", dockerfilePath: "packages/deck-builder/Dockerfile",
      watchPatterns: ["/packages/deck-builder/**", "/package.json", "/package-lock.json", "/.dockerignore"],
    },
    replicas: { ams: 1 }, healthcheck: "/api/health", healthcheckTimeout: 300,
    start: "sh /usr/local/bin/deck-builder-entrypoint node src/server.ts",
    volumeMounts: { "/data": data },
    env: {
      NODE_ENV: "production", DECKBUILDER_MODE: "hosted", DECKBUILDER_DATA: "/data",
      DECKBUILDER_PORT: "8787", PORT: "8787", DECKBUILDER_TRUST_PROXY: "1",
      DECKBUILDER_INVITES_REQUIRED: "1", RAILWAY_RUN_UID: "0",
      DECKBUILDER_PUBLIC_ORIGIN: preserve(),
      DECKBUILDER_OPERATOR_NAME: preserve(), DECKBUILDER_CONTACT_EMAIL: preserve(),
      DISCORD_CLIENT_ID: preserve(), DISCORD_CLIENT_SECRET: preserve(), BETTER_AUTH_SECRET: preserve(),
      BACKUP_S3_ENDPOINT: preserve(), BACKUP_S3_BUCKET: preserve(),
      BACKUP_S3_ACCESS_KEY: preserve(), BACKUP_S3_SECRET_KEY: preserve(),
      ...(production ? {} : { DECKBUILDER_CLAIM_CODE: preserve() }),
      // Claim code is bootstrap-only. Never recreate it after the owner claim.
    },
  });
  return project("deck-builder", { resources: [data, backups, app] });
});
