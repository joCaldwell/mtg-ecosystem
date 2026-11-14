import type { ActionBeginning, ActionPage, ActionScope, PlayerResponse } from '@multiverse/game-server-sdk';
import type { ActionQueryContext } from './action-query.ts';
import type { CommandContext } from './command.ts';

export type LiveActionScope = ActionScope;

/** In-memory original intent. Never replace either context at completion time. */
export interface LiveActionDraft {
  readonly command: CommandContext;
  readonly query: ActionQueryContext;
  readonly scope: ActionScope;
  readonly generation: number;
}
/** Pass is self-explanatory in an authoritative priority window. Every object,
 * ability and payment beginning still needs qualified human source/label context.
 * Presence in discovery is initiation validation, never inferred source meaning. */
export function actionResponse(beginning: Readonly<ActionBeginning>, context: ActionPage['context'], scope: ActionScope | null): PlayerResponse | null {
  return scope?.scope === 'window' && context?.['window'] === 'priority' && beginning.initiation.action === 'pass' ? { response: 'pass' } : null;
}
export function actionDescription(beginning: Readonly<ActionBeginning>): string {
  switch (beginning.initiation.action) {
    case 'pass': return 'Pass priority';
    case 'play_land': return 'Play land — card context unavailable';
    case 'cast': return 'Cast spell — card and payment context unavailable';
    case 'activate_mana': return 'Activate mana — ability label unavailable';
    case 'activate_ability': return 'Activate ability — source label unavailable';
    case 'finish_mana': return 'Finish mana — payment context unavailable';
  }
}
