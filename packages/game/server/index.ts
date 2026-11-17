import type { ApplicationFactory } from "@mtg-ecosystem/deck-builder/application";
import type { GameCoordinatorOptions } from "../shared/contracts.ts";
import { createGameCoordinator } from "./games.ts";
import { handleGameRequest } from "./routes.ts";
export { nativeLifecycleFromEnvironment } from "./native-lifecycle.ts";
export { nativeAdmissionFromEnvironment } from "./native-admission.ts";
export function gameApplication(options: GameCoordinatorOptions = {}): ApplicationFactory {
  return ports => {
    const games = createGameCoordinator(ports, options);
    return {
      matches: url => url.pathname === "/api/games" || url.pathname.startsWith("/api/games/"),
      handle: request => handleGameRequest(games, request),
      start: () => games.start(),
      close: () => games.close(),
    };
  };
}
