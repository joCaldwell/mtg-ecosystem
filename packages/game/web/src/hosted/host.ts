import type { ComponentType, ReactNode } from "react";
import type { GameAdmission, GameLaunchResult, GameRecord } from "../../../shared/contracts.ts";
/** Product navigation, HTTP and visual primitives; no product-private imports. */
export interface GameBrowserHost {
  /** Share the hosted game proxy's requests with the product transport. */
  requestFetch?: typeof fetch;
  startRequestBudget?: () => () => void;
  api: {
    gameAdmission(deckId: number, revision: number): Promise<{ admission: GameAdmission }>;
    launchGame(deckId: number, revision: number, requestId: string): Promise<GameLaunchResult>;
    resumeGame(id: string): Promise<GameLaunchResult>;
    getGame(id: string, signal: AbortSignal): Promise<{ game: GameRecord }>;
    humanGameSeat(id: string, signal: AbortSignal): Promise<{ bootstrap_file: string }>;
  };
  ApiError: new (status: number, message: string) => Error & { status: number };
  navigateGame(id: string): void;
  backHref: string;
  useSavedGames(): { data: { games: GameRecord[] } | null; error: string | null; loading: boolean; reload(): void };
  ui: {
    Button: ComponentType<{ children?: ReactNode; disabled?: boolean; onClick?: () => void; variant?: "primary"; "aria-describedby"?: string }>;
    ButtonLink: ComponentType<{ children?: ReactNode; href: string }>;
    ErrorNote: ComponentType<{ children?: ReactNode }>;
    Loading: ComponentType;
    Page: ComponentType<{ children: ReactNode; width: "full"; header: ReactNode }>;
    PageHeader: ComponentType<{ title: string; actions: ReactNode }>;
    Text: ComponentType<{ children?: ReactNode; as?: "p"; tone?: "muted"; mono?: boolean }>;
  };
}
