/** Finite all-White Commander development policy. Only permitted DTOs enter it. */
import type { Observation, PlayerResponse, PublicManifest } from '../client.ts';
import type { ManaCost, VisibleObject } from '../../generated/stream.js';

function fail(): never { throw new Error('commander_policy_unsupported'); }
function quantity(value: string): bigint {
  if (!/^(0|-?[1-9][0-9]*)$/.test(value)) return fail();
  return BigInt(value);
}
function whiteCost(cost: ManaCost): bigint {
  const generic = quantity(cost.generic);
  if (generic < 0n || cost.colored.some(color => color !== 'W')) return fail();
  return generic + BigInt(cost.colored.length);
}
function firstHandles(candidates: string[], count: number): string[] {
  if (!Number.isSafeInteger(count) || count < 0 || count > candidates.length
    || new Set(candidates).size !== candidates.length) return fail();
  return candidates.slice(0, count);
}

/**
 * The host/transport validates schemas; this policy supports only the admitted
 * all-Isamaru foundation pod. It casts no optional spells or activated abilities.
 * Public commander history supplies tax; quantities never pass through Number.
 */
export function commanderDevelopmentPolicy(
  observation: Observation, viewer: number, manifest: PublicManifest,
): PlayerResponse | null {
  const view = observation.view;
  if (view.viewer !== viewer || !Number.isSafeInteger(viewer) || viewer < 0 || viewer > 3) return fail();
  const request = observation.decision;
  if (request === null) return null;
  if (request.player !== viewer || manifest.profile !== 'commander-foundations-v1') return fail();
  const commanderId = manifest.decks[viewer]?.commander;
  const commander = manifest.catalog.definitions.find(card => card.oracle_id === commanderId);
  if (commander === undefined || commander.name !== 'Isamaru, Hound of Konda'
    || manifest.decks.some(deck => deck.commander !== commanderId)) return fail();
  const plains = new Set(manifest.catalog.definitions.filter(card => card.characteristics.kind === 'land'
    && card.characteristics.basic && card.characteristics.subtypes.includes('Plains')
    && card.intrinsic_zone === 'battlefield').map(card => card.oracle_id));
  const untappedPlains = (): VisibleObject[] => view.objects.filter(card => card.zone.zone === 'battlefield'
    && card.controller === viewer && !card.tapped && plains.has(card.oracle_id));
  const ownPool = view.players.find(player => player.player === viewer)?.mana;
  const white = (): bigint => {
    if (ownPool === undefined) return fail();
    const amount = quantity(ownPool.W ?? '0');
    return amount < 0n ? fail() : amount;
  };
  switch (request.request) {
    case 'mulligan': return { response: 'mulligan', take: false };
    case 'bottom_opening': return { response: 'bottom_opening', objects: firstHandles(request.candidates, request.count) };
    case 'priority': {
      if (view.active !== viewer || view.priority !== viewer || view.stack.length !== 0
        || !['precombat_main', 'postcombat_main'].includes(view.turn.step)) return { response: 'pass' };
      if (view.turn.lands_played === 0) {
        const land = view.objects.find(card => card.owner === viewer && card.zone.zone === 'hand'
          && card.zone.owner === viewer && plains.has(card.oracle_id));
        if (land !== undefined) return { response: 'play_land', object: land.handle };
      }
      const card = view.objects.find(card => card.oracle_id === commanderId && card.owner === viewer
        && card.zone.zone === 'command' && card.zone.owner === viewer);
      if (card === undefined) return { response: 'pass' };
      const history = view.commanders.find(row => row.oracle_id === commanderId && row.history.owner === viewer);
      if (history === undefined) return fail();
      const casts = quantity(history.history.command_casts[String(viewer)] ?? '0');
      if (casts < 0n) return fail();
      const required = whiteCost(commander.characteristics.mana_cost) + 2n * casts;
      return white() + BigInt(untappedPlains().length) >= required
        ? { response: 'cast', card: card.handle } : { response: 'pass' };
    }
    case 'mana': {
      const required = whiteCost(request.cost);
      if (white() >= required) return { response: 'finish_mana', undo_mana: true };
      const land = untappedPlains()[0];
      // A departure can remove funding after announcement. FinishMana performs
      // the core's legal reversal; a later priority decision reevaluates funds.
      return land === undefined ? { response: 'finish_mana', undo_mana: true }
        : { response: 'activate_mana', object: land.handle };
    }
    case 'spend_mana': {
      const required = whiteCost(request.cost);
      if (quantity(request.available.W ?? '0') < required) return fail();
      return { response: 'spend_mana', spending: required === 0n ? {} : { W: required.toString() } };
    }
    case 'attackers': {
      const defenders = request.defenders.filter(player => player !== viewer && view.live_turn_order.includes(player));
      if (defenders.some(player => !Number.isSafeInteger(player) || player < 0 || player > 3)) return fail();
      defenders.sort((a, b) => (a - viewer + 4) % 4 - (b - viewer + 4) % 4);
      const defender = defenders[0];
      const attacks: [string, number][] = [];
      if (defender !== undefined) for (const handle of request.candidates) {
        const card = view.objects.find(card => card.handle === handle);
        if (card !== undefined && card.controller === viewer && card.oracle_id === commanderId
          && card.zone.zone === 'battlefield' && card.attack_ready && !card.tapped
          && card.characteristics?.power !== null && card.characteristics?.power !== undefined
          && quantity(card.characteristics.power) > 0n) attacks.push([handle, defender]);
      }
      return { response: 'attackers', attacks };
    }
    case 'blockers': return { response: 'blockers', blocks: [] };
    case 'commander_return': return { response: 'commander_return', to_command: true };
    case 'discard': return { response: 'discard', objects: firstHandles(request.candidates, request.count) };
    case 'legend_rule': return fail();
    default: return fail();
  }
}
