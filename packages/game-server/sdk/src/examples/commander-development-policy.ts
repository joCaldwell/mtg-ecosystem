/** Finite all-White Commander development policy. Only permitted DTOs enter it. */
import type { Candidate, Observation, Answer, PublicManifest } from '../client.ts';
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
/**
 * The host/transport validates schemas; this policy supports only the admitted
 * all-Isamaru foundation pod. It casts no optional spells or activated abilities.
 * Public commander history supplies tax; quantities never pass through Number.
 */
export function commanderDevelopmentPolicy(
  observation: Observation, viewer: number, manifest: PublicManifest,
): Answer | null {
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
  const {shape,candidates} = request;
  function pick(test:(candidate:Candidate)=>boolean):Answer {
    if (shape.shape !== 'pick' || shape.slots.length !== 1) return fail();
    const i=shape.slots[0]?.allowed.find(i => candidates[i] !== undefined && test(candidates[i]!));
    return i === undefined ? fail() : {answer:'pick', picks:[[i]]};
  }
  const pass = () => pick(c=>c.candidate==='action' && c.action.action==='pass');
  switch(request.reason.reason){
    case 'mulligan': return pick(c=>c.candidate==='flag' && c.flag==='keep');
    case 'commander_return': return pick(c=>c.candidate==='flag' && c.flag==='command_zone');
    case 'bottom_opening': return shape.shape==='order' ? {answer:'order',order:shape.items.slice(0,shape.min)} : fail();
    case 'priority': {
      if (view.active !== viewer || view.priority !== viewer || view.stack.length !== 0
        || !['precombat_main', 'postcombat_main'].includes(view.turn.step)) return pass();
      if (view.turn.lands_played === 0) {
        const land = view.objects.find(card => card.owner === viewer && card.zone.zone === 'hand'
          && card.zone.owner === viewer && plains.has(card.oracle_id));
        if (land !== undefined) return pick(c=>c.candidate==='action' && c.action.action==='play_land' && c.action.object===land.handle);
      }
      const card = view.objects.find(card => card.oracle_id === commanderId && card.owner === viewer
        && card.zone.zone === 'command' && card.zone.owner === viewer);
      if (card === undefined) return pass();
      const history = view.commanders.find(row => row.oracle_id === commanderId && row.history.owner === viewer);
      if (history === undefined) return fail();
      const casts = quantity(history.history.command_casts[String(viewer)] ?? '0');
      if (casts < 0n) return fail();
      const required = whiteCost(commander.characteristics.mana_cost) + 2n * casts;
      return white() + BigInt(untappedPlains().length) >= required
        ? pick(c=>c.candidate==='action' && c.action.action==='cast' && c.action.card===card.handle) : pass();
    }
    case 'mana': {
      if(shape.shape!=='pay' || shape.step.step!=='produce') return fail();
      const source=shape.step.sources.find(i=> {
        const c=candidates[i];
        return c?.candidate==='action' && c.action.action==='activate_mana' && untappedPlains().some(land=>land.handle===('object' in c.action && c.action.object));
      });
      if(white()>=whiteCost(shape.cost) || source===undefined) return {answer:'pay',pay:{step:'finish',undo_on_failure:true}};
      return {answer:'pay',pay:{step:'activate',source}};
    }
    case 'spend_mana': {
      if(shape.shape!=='pay' || shape.step.step!=='spend') return fail();
      const required=whiteCost(shape.cost);
      if(quantity(shape.pool.unrestricted.W ?? '0')<required) return fail();
      return {answer:'pay',pay:{step:'spend',unrestricted:required===0n?{}:{W:String(required)}}};
    }
    case 'attackers': {
      if(shape.shape!=='pick') return fail();
      return {answer:'pick',picks:shape.slots.map(slot=>{
        const c=candidates[slot.subject ?? -1];
        const card=c?.candidate==='object' && view.objects.find(o=>o.handle===c.object);
        if(!card || card.oracle_id!==commanderId || !card.attack_ready || card.tapped || quantity(card.characteristics?.power ?? '0')<=0n) return [];
        const players=slot.allowed.flatMap(i=>{const c=candidates[i];return c?.candidate==='player'?[{i,p:c.player}]:[];});
        players.sort((a,b)=>(a.p-viewer+4)%4-(b.p-viewer+4)%4);
        return players[0]?[players[0].i]:[];
      })};
    }
    case 'discard':
    case 'blockers': return shape.shape==='pick' ? {answer:'pick',picks:shape.slots.map(s=>s.allowed.slice(0,s.min))} : fail();
    default: return fail();
  }
}
