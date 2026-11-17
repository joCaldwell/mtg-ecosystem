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
/** Source-dependent beginnings require the producer's qualified explanation.
 * These checks never derive affordability or later target/payment legality. */
export function actionResponse(beginning: Readonly<ActionBeginning>, context: ActionPage['context'], scope: ActionScope | null): PlayerResponse | null {
  const action = beginning.initiation;
  if (action.action === 'pass') return scope?.scope === 'window' && context?.['window'] === 'priority' ? { response: 'pass' } : null;
  if (!scope || !context || beginning.label === 'unavailable' || !beginning.label.available.text) return null;
  const source = beginning.label.available.source;
  const boundObject = (handle: string) => scope.scope === 'window' || scope.object === handle;
  switch (action.action) {
    case 'play_land': return context['window'] === 'priority' && boundObject(action.object) ? { response: 'play_land', object: action.object } : null;
    case 'cast': return context['window'] === 'priority' && boundObject(action.card) && source?.kind === 'spell' && source.object === action.card ? { response: 'cast', card: action.card } : null;
    case 'activate_ability': return context['window'] === 'priority' && boundObject(action.object) && source?.kind === 'activated' && source.object === action.object && source.ability === action.ability && source.text ? { response: 'activate_ability', object: action.object, ability: action.ability } : null;
    case 'activate_mana': return boundObject(action.object) && source?.kind === 'mana_ability' && source.object === action.object && source.text ? { response: 'activate_mana', object: action.object } : null;
    case 'finish_mana': return context['window'] === 'mana' && scope.scope === 'window' && context.source.status === 'available' ? { response: 'finish_mana', undo_mana: action.undo_mana } : null;
  }
}
export function actionDescription(beginning: Readonly<ActionBeginning>, objects: readonly { readonly handle: string; readonly name: string | null }[] = []): string {
  if (beginning.label !== 'unavailable') {
    const action = beginning.initiation;
    const handle = 'object' in action ? action.object : 'card' in action ? action.card : null;
    return `${beginning.label.available.text}${handle ? ` · ${objects.find(object => object.handle === handle)?.name ?? 'Unidentified object'} (${handle})` : ''}`;
  }
  switch (beginning.initiation.action) {
    case 'pass': return 'Pass priority';
    case 'play_land': return 'Play land — card context unavailable';
    case 'cast': return 'Cast spell — card and payment context unavailable';
    case 'activate_mana': return 'Activate mana — ability label unavailable';
    case 'activate_ability': return 'Activate ability — source label unavailable';
    case 'finish_mana': return 'Finish mana — payment context unavailable';
  }
}
