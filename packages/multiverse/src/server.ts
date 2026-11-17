import { fileURLToPath } from "node:url";
import { createHost } from "@mtg-ecosystem/deck-builder/host";
import { gameApplication, nativeLifecycleFromEnvironment, nativeAdmissionFromEnvironment } from "@multiverse/game/server";
import { isApplicationPath } from "./page-paths.ts";
import { isQualifiedProductSession } from "@multiverse/game/session-qualification";

// Storage/auth configuration remains in the existing backend service. The product
// chooses its frontend, page surface and process lifecycle; no second account DB.
try {
  const native = nativeLifecycleFromEnvironment() ?? nativeAdmissionFromEnvironment();
  const host = await createHost({ webDist: fileURLToPath(new URL("../web/dist/", import.meta.url)), isApplicationPath, application: gameApplication({ native, browser_session_qualified: isQualifiedProductSession(native?.identity) }) });
  let stopping: Promise<void> | undefined;
  const stop = (failed = false) => {
    if (failed) process.exitCode = 1;
    return stopping ??= host.close();
  };
  process.once("SIGTERM", () => { void stop(); });
  process.once("SIGINT", () => { void stop(); });
  host.server.on("error", () => {
    console.error(JSON.stringify({ event: "application_server_error" }));
    void stop(true);
  });
  const { port, mode } = await host.start();
  console.log(JSON.stringify({ event: "application_started", application: "Multiverse", port, mode, pid: process.pid }));
} catch {
  // Configuration and authentication errors can contain secrets. Detailed backend
  // logging owns redaction; the process entrypoint reports a bounded failure.
  console.error(JSON.stringify({ event: "application_start_failed" }));
  process.exitCode = 1;
}
