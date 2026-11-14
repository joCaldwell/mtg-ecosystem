import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { verifyPreviewFixtures } from "@multiverse/game-client/verify-preview";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

// DECKBUILDER_PORT moves the API server (scripts/dev.mjs passes it on), so the
// proxy follows it rather than assuming 8787.
const apiPort = Number(process.env.DECKBUILDER_PORT ?? 8787);

export default defineConfig({
  root: "web",
  // Moving product source must not move or silently replace operator dotenv
  // configuration. Preserve the original Vite environment-file location.
  envDir: resolve(dirname(fileURLToPath(import.meta.url)), "../deck-builder/web"),
  // Shared source packages use the host React runtime, never their local harness installs.
  resolve: { dedupe: ["react", "react-dom"] },
  plugins: [react(), {
    name: "qualified-game-preview",
    async buildStart() { await verifyPreviewFixtures(); },
  }],
  build: { outDir: "dist", manifest: true, rollupOptions: {
    // These barrels contain re-exports only (enforced by architecture tests).
    // Discard unused exports before following the editor's dependency graph.
    treeshake: { moduleSideEffects: id => !/web\/src\/(features\/[^/]+|mtg|ui)\/index\.ts$/.test(id) },
    output: { manualChunks: { react: ["react", "react-dom", "react-dom/client", "react/jsx-runtime"] } } } },
  server: {
    port: Number(process.env.DECKBUILDER_WEB_PORT ?? 5173),
    proxy: { "/api": `http://localhost:${apiPort}` },
  },
});
