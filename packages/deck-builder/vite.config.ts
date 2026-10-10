import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// DECKBUILDER_PORT moves the API server (scripts/dev.mjs passes it on), so the
// proxy follows it rather than assuming 8787.
const apiPort = Number(process.env.DECKBUILDER_PORT ?? 8787);

export default defineConfig({
  root: "web",
  plugins: [react()],
  build: { outDir: "dist" },
  server: {
    port: 5173,
    proxy: { "/api": `http://localhost:${apiPort}` },
  },
});
