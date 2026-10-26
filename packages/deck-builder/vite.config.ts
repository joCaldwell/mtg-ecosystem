import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// DECKBUILDER_PORT moves the API server (scripts/dev.mjs passes it on), so the
// proxy follows it rather than assuming 8787.
const apiPort = Number(process.env.DECKBUILDER_PORT ?? 8787);

export default defineConfig({
  root: "web",
  plugins: [react()],
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
