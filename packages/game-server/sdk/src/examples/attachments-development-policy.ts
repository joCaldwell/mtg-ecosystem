/** Finite completion policy, ported from sim/match_games/{attachments,mana,four_colors}_policy.rs.
 * Inputs are exclusively this seat's Observation and the public manifest/catalog.
 * Make one instance per seat/game; planned mana never crosses consumers.
 */
import type { Observation, PlayerResponse, PublicManifest } from '../client.ts';
import type { Color, ManaCost, VisibleObject } from '../../generated/stream.js';

type Pool = Partial<Record<Color, string>>;
type Plan = { source: string; color?: Color }[];
const colors: Color[] = ['B', 'G', 'W', 'U', 'R', 'C'];
const zero: ManaCost = { generic: '0', colored: [] };
function fail(): never { throw new Error('attachments_policy_unsupported'); }
const q = (s: string): bigint => /^(0|-?[1-9][0-9]*)$/.test(s) ? BigInt(s) : fail();
const first = <T>(rows: T[]): T => rows[0] ?? fail();
const take = <T>(rows: T[], count: number): T[] => Number.isSafeInteger(count) && count >= 0 && count <= rows.length ? rows.slice(0, count) : fail();
const min = (a: bigint, b: bigint): bigint => a < b ? a : b;
/** Exact colored requirements first, then deterministic generic spending. */
export function attachmentsSpend(cost: ManaCost, pool: Pool): Pool | null {
  const free = Object.fromEntries(colors.map(c => [c, q(pool[c] ?? '0')])) as Record<Color, bigint>;
  const used: Pool = {};
  const add = (c: Color, n: bigint): void => { if (n > 0n) used[c] = (q(used[c] ?? '0') + n).toString(); };
  for (const c of cost.colored) { if (free[c] < 1n) return null; free[c]--; add(c, 1n); }
  let rest = q(cost.generic);
  if (rest < 0n || colors.some(c => free[c] < 0n)) return fail();
  for (const c of colors) { const n = min(free[c], rest); rest -= n; add(c, n); }
  return rest === 0n ? used : null;
}

export function attachmentsDevelopmentPolicy(): (observation: Observation, viewer: number, manifest: PublicManifest) => PlayerResponse | null {
  let manaPlan: Plan = [];
  let manaColor: Color | undefined;
  let beneficial = false;
  let identity: string | undefined;
  return (observation, viewer, manifest) => {
    const v = observation.view, r = observation.decision;
    if (!Number.isInteger(viewer) || viewer < 0 || viewer > 3 || v.viewer !== viewer
      || (r !== null && r.player !== viewer) || manifest.profile !== 'commander-attachments-v1'
      || manifest.catalog.schema_version !== 17 || manifest.catalog.definitions.length !== 118) return fail();
    const binding = `${manifest.match_id}:${viewer}:${manifest.engine_build_id}:${manifest.catalog_sha256}`;
    if (identity !== undefined && identity !== binding) return fail();
    identity = binding;
    if (r === null) return null;
    const d = (o: VisibleObject) => manifest.catalog.definitions.find(d => d.oracle_id === o.oracle_id) ?? fail();
    const object = (h: string): VisibleObject => v.objects.find(o => o.handle === h) ?? fail();
    const name = (o: VisibleObject): string => o.token ? o.token.name : d(o).name;
    const own = (o: VisibleObject): boolean => o.controller === viewer;
    const battlefield = (o: VisibleObject): boolean => o.zone.zone === 'battlefield';
    const creature = (o: VisibleObject): boolean => o.characteristics?.kind === 'creature';
    const power = (o: VisibleObject): bigint => q(o.characteristics?.power ?? '0');
    const commander = (o: VisibleObject): boolean => v.commanders.some(c => c.oracle_id === o.oracle_id);
    const targetable = (o: VisibleObject): boolean => !(o.keywords ?? []).includes('shroud') && (own(o) || !(o.keywords ?? []).includes('hexproof'));
    const best = (rows: VisibleObject[], preferCommander = false): VisibleObject | undefined => rows.toSorted((a, b) => {
      const ac = preferCommander && commander(a), bc = preferCommander && commander(b);
      if (ac !== bc) return ac ? -1 : 1;
      return power(a) === power(b) ? (a.handle < b.handle ? 1 : -1) : power(a) > power(b) ? -1 : 1;
    })[0];
    const pool = (): Pool => v.players.find(p => p.player === viewer)?.mana ?? fail();
    const plan = (cost: ManaCost, exclude?: string): Plan | null => {
      const commanderDefinition = manifest.catalog.definitions.find(c => c.oracle_id === manifest.decks[viewer]?.commander);
      const sources = v.objects.filter(o => battlefield(o) && own(o) && !o.tapped && !o.token && o.handle !== exclude)
        .flatMap(o => {
          const def = d(o);
          if (creature(o) && !o.attack_ready && !(o.keywords ?? []).includes('haste')) return [];
          const ability = def.mana_abilities?.[0];
          const production = ability?.production ?? (def.characteristics.kind === 'land' && def.characteristics.basic
            ? { mana: 'fixed' as const, symbols: def.characteristics.color_identity } : undefined);
          if (production === undefined) return [];
          const cs: Color[] = production.mana === 'fixed' ? production.symbols : production.mana === 'any_color'
            ? ['W', 'U', 'B', 'R', 'G'] : commanderDefinition?.characteristics.color_identity ?? [];
          return cs.length === 0 ? [] : [{ o, land: def.characteristics.kind === 'land', production, colors: cs, cost: ability?.costs.mana }];
        });
      const used = new Set<number>(), result: Plan = [];
      const funds: Pool = { ...pool() };
      const fund = (cost: ManaCost): boolean => {
        for (let step = 0; step <= sources.length; step++) {
          if (attachmentsSpend(cost, funds) !== null) return true;
          const needed = Object.fromEntries(colors.map(c => [c, BigInt(cost.colored.filter(x => x === c).length)])) as Record<Color, bigint>;
          const deficit = colors.find(c => q(funds[c] ?? '0') < needed[c]);
          const i = sources.map((s, i) => ({ s, i })).filter(({ s, i }) => !used.has(i) && (deficit === undefined || s.colors.includes(deficit)))
            .toSorted((a, b) => Number(a.s.land) - Number(b.s.land) || Number(a.s.cost != null) - Number(b.s.cost != null) || a.i - b.i)[0]?.i;
          if (i === undefined) return false;
          used.add(i);
          const source = sources[i]!;
          if (source.cost != null) {
            if (!fund(source.cost)) return false;
            const spending = attachmentsSpend(source.cost, funds) ?? fail();
            for (const c of colors) funds[c] = (q(funds[c] ?? '0') - q(spending[c] ?? '0')).toString();
          }
          const color = source.production.mana === 'fixed' ? undefined : deficit ?? source.colors[0];
          const symbols = source.production.mana === 'fixed' ? source.production.symbols : [color ?? fail()];
          for (const c of symbols) funds[c] = (q(funds[c] ?? '0') + 1n).toString();
          result.push({ source: source.o.handle, ...(color === undefined ? {} : { color }) });
        }
        return false;
      };
      return fund(cost) ? result : null;
    };
    const announce = (response: PlayerResponse, cost: ManaCost, good = false, exclude?: string): PlayerResponse | null => {
      const p = plan(cost, exclude);
      if (p === null) return null;
      manaPlan = p; beneficial = good;
      return response;
    };
    const stackTarget = (ability: boolean, candidates?: string[]): string | undefined => v.stack.filter(s => s.controller !== viewer && s.cast_complete
      && s.handle != null && (candidates === undefined || candidates.includes(s.handle)))
      .filter(s => ability ? s.ability != null || s.triggered != null : s.spell != null).at(-1)?.handle ?? undefined;
    const top = v.stack.at(-1);
    const topDefinition = top?.spell ? d(object(top.spell)) : top?.ability
      ? manifest.catalog.definitions.find(d => d.oracle_id === top.ability?.oracle_id) : undefined;
    switch (r.request) {
      case 'mulligan': return { response: 'mulligan', take: false };
      case 'bottom_opening': return { response: 'bottom_opening', objects: take(r.candidates, r.count) };
      case 'priority': {
        const hand = v.objects.filter(o => o.owner === viewer && o.zone.zone === 'hand' && o.zone.owner === viewer);
        for (const cardName of ['Counterspell', 'Stifle']) {
          if (stackTarget(cardName === 'Stifle') === undefined) continue;
          const card = hand.find(o => name(o) === cardName);
          if (card) { const response = announce({ response: 'cast', card: card.handle }, d(card).characteristics.mana_cost); if (response) return response; }
        }
        if (v.active !== viewer || v.stack.length !== 0 || !['precombat_main', 'postcombat_main'].includes(v.turn.step)) return { response: 'pass' };
        if (v.turn.lands_played === 0) {
          const land = hand.find(o => d(o).characteristics.kind === 'land');
          if (land) return { response: 'play_land', object: land.handle };
        }
        // Retain attachments on commanders and avoid zero-cost re-equip loops.
        for (const equipment of ['Swiftfoot Boots', 'Lightning Greaves', 'Loxodon Warhammer', 'Whispersilk Cloak', 'Vulshok Morningstar', 'Bonesplitter', 'Short Sword']) {
          const source = v.objects.find(o => battlefield(o) && own(o) && !o.token && name(o) === equipment);
          const prefer = ['Swiftfoot Boots', 'Lightning Greaves'].includes(equipment);
          const target = best(v.objects.filter(o => battlefield(o) && own(o) && creature(o) && targetable(o)), prefer);
          if (!source || !target || source.attached_to === target.handle) continue;
          const current = source.attached_to ? object(source.attached_to) : undefined;
          if (current && ((prefer && commander(current)) || (own(current) && power(current) >= power(target) && !(prefer && commander(target))))) continue;
          const cost = d(source).activated?.[0]?.cost;
          if (cost === undefined || typeof cost === 'string' || !('mana' in cost)) return fail();
          const response = announce({ response: 'activate_ability', object: source.handle, ability: 0 }, cost.mana.cost, true);
          if (response) return response;
        }
        const enemy = v.objects.some(o => battlefield(o) && !own(o) && creature(o) && targetable(o));
        const friendly = v.objects.some(o => battlefield(o) && own(o) && creature(o) && targetable(o));
        for (const command of [true, false]) for (const card of v.objects) {
          if (card.owner !== viewer || card.zone.zone !== (command ? 'command' : 'hand') || !('owner' in card.zone) || card.zone.owner !== viewer) continue;
          const def = d(card), kind = def.characteristics.kind, n = def.name;
          if (kind === 'land' || ['Fog', 'Counterspell', 'Stifle'].includes(n)) continue;
          if (!command && !['creature', 'enchantment', 'artifact'].includes(kind) && v.turn.step !== 'postcombat_main') continue;
          if (['Cruel Edict', 'Disfigure', 'Murder', 'Unsummon', 'Time Ebb', 'Pacifism'].includes(n) && !enemy) continue;
          if (['Giant Growth', 'Battlegrowth'].includes(n) && !friendly) continue;
          if (def.characteristics.subtypes.includes('Aura') && !(n === 'Pacifism' ? enemy : friendly)) continue;
          if (n === 'Day of Judgment' && v.objects.filter(o => battlefield(o) && creature(o) && !own(o)).length <= v.objects.filter(o => battlefield(o) && creature(o) && own(o)).length) continue;
          if (n === 'Raise Dead' && !v.objects.some(o => o.zone.zone === 'graveyard' && o.zone.owner === viewer && !o.token && d(o).characteristics.kind === 'creature')) continue;
          const cost = structuredClone(def.characteristics.mana_cost);
          if (command) {
            const history = v.commanders.find(c => c.oracle_id === card.oracle_id && c.history.owner === viewer) ?? fail();
            cost.generic = (q(cost.generic) + 2n * q(history.history.command_casts[String(viewer)] ?? '0')).toString();
          }
          const response = announce({ response: 'cast', card: card.handle }, cost, ['Giant Growth', 'Battlegrowth', 'Healing Hands', 'Healing Salve', 'Sacred Nectar'].includes(n));
          if (response) return response;
        }
        // Late draw/ability use follows the simulator; never draw from an empty library intentionally.
        if (v.turn.step === 'postcombat_main' && v.turn.number >= 20 && (v.players.find(p => p.player === viewer)?.library_count ?? 0) > 1) {
          for (const o of v.objects.filter(o => battlefield(o) && own(o) && !o.token)) {
            const def = d(o);
            if (!['Merfolk Looter', 'Azure Mage', 'Hapless Researcher', 'Prodigal Sorcerer', 'Mind Stone', "Commander's Sphere"].includes(def.name)) continue;
            for (const [ability, a] of (def.activated ?? []).entries()) {
              let cost = zero;
              if (a.cost === 'tap_source') { if (o.tapped || !o.attack_ready) continue; }
              else if (typeof a.cost !== 'string' && 'mana' in a.cost) cost = a.cost.mana.cost;
              else if (typeof a.cost !== 'string' && 'composite' in a.cost) { if (a.cost.composite.costs.tap && o.tapped) continue; cost = a.cost.composite.costs.mana ?? zero; }
              else if (a.cost !== 'sacrifice_source') continue;
              const response = announce({ response: 'activate_ability', object: o.handle, ability }, cost, false, o.handle);
              if (response) return response;
            }
          }
        }
        return { response: 'pass' };
      }
      case 'mana': {
        if (attachmentsSpend(r.cost, pool()) !== null) return { response: 'finish_mana', undo_mana: true };
        // Re-plan from the current public view if a separately issued mana request has no retained plan.
        if (manaPlan.length === 0) manaPlan = plan(r.cost, r.mana_ability_source ?? undefined) ?? [];
        const next = manaPlan.shift();
        manaColor = next?.color;
        return next ? { response: 'activate_mana', object: next.source } : { response: 'finish_mana', undo_mana: true };
      }
      case 'mana_color': {
        const color = manaColor !== undefined && r.colors.includes(manaColor) ? manaColor : first(r.colors);
        manaColor = undefined;
        return { response: 'mana_color', color };
      }
      case 'spend_mana': return { response: 'spend_mana', spending: attachmentsSpend(r.cost, r.available) ?? fail() };
      case 'targets': {
        const attachment = topDefinition?.characteristics.subtypes.some(s => ['Aura', 'Equipment'].includes(s));
        const recursion = r.occurrences[0]?.domain === 'own_graveyard_creature_card';
        const counterTrigger = top?.triggered && manifest.catalog.definitions.find(d => d.oracle_id === top.triggered?.oracle_id)?.triggered?.[top.triggered.ability]?.program.instructions.some(i => i.op === 'put_plus_one_counters');
        const good = attachment ? topDefinition?.name !== 'Pacifism' : recursion || counterTrigger || (beneficial && !top?.triggered);
        const target = best(r.candidates.map(object).filter(o => recursion || own(o) === Boolean(good)), attachment && ['Swiftfoot Boots', 'Lightning Greaves'].includes(topDefinition?.name ?? ''))?.handle ?? first(r.candidates);
        return { response: 'targets', objects: r.occurrences.map(() => target) };
      }
      case 'player_targets': return { response: 'player_targets', players: r.occurrences.map(() => r.candidates.find(p => (p === viewer) === (beneficial && !top?.triggered)) ?? first(r.candidates)) };
      case 'entity_targets': {
        const enemy = best(r.candidates.flatMap(e => e.entity === 'object' ? [object(e.object)] : []).filter(o => !own(o) && power(o) > 0n));
        const target = enemy ? r.candidates.find(e => e.entity === 'object' && e.object === enemy.handle)! : r.candidates.filter(e => e.entity === 'player' && e.player !== viewer).toSorted((a, b) => {
          const life = (e: typeof a): bigint => e.entity === 'player' ? q(v.players.find(p => p.player === e.player)?.life ?? '0') : 0n;
          return life(a) === life(b) ? 0 : life(a) < life(b) ? -1 : 1;
        })[0] ?? first(r.candidates);
        return { response: 'entity_targets', entities: r.occurrences.map(() => target) };
      }
      case 'stack_targets': return { response: 'stack_targets', entries: r.occurrences.map(() => stackTarget(r.occurrences[0]?.domain === 'stack_activated_or_triggered_ability', r.candidates) ?? first(r.candidates)) };
      case 'modes': return { response: 'modes', mode: first(r.candidates) };
      case 'order_triggers': return { response: 'order_triggers', order: r.candidates.map(c => c.handle) };
      case 'search_lands': return { response: 'search_lands', objects: r.candidates.slice(0, r.maximum).map(c => c.handle) };
      case 'optional_recursion': return { response: 'optional_recursion', perform: true };
      // Removal-profile only; this policy is pinned to the attachments catalog.
      case 'optional_effect': return fail();
      case 'commander_return': return { response: 'commander_return', to_command: true };
      case 'sacrifice': return { response: 'sacrifice', object: first(r.candidates) };
      case 'discard': return { response: 'discard', objects: take(r.candidates, r.count) };
      case 'resolve_discard': return { response: 'resolve_discard', objects: take(r.candidates, r.count) };
      case 'attackers': {
        const defender = r.defenders.toSorted((a, b) => (a + 4 - viewer) % 4 - (b + 4 - viewer) % 4)[0];
        return { response: 'attackers', attacks: defender === undefined ? [] : r.candidates.filter(h => power(object(h)) > 0n).map(h => [h, defender]) };
      }
      case 'blockers': {
        const blocks: [string, string][] = [], blocked = new Set<string>();
        for (const hint of r.legal_attackers ?? []) {
          const b = object(hint.blocker);
          const attacker = hint.attackers.find(h => {
            const a = object(h), p = power(a) > 0n ? power(a) : 0n;
            return !blocked.has(h) && ((q(b.marked_damage) + p < q(b.characteristics?.toughness ?? '0') && (p === 0n || !(a.keywords ?? []).includes('deathtouch'))) || (b.keywords ?? []).includes('deathtouch'));
          });
          if (attacker) { blocks.push([hint.blocker, attacker]); blocked.add(attacker); }
        }
        for (const attacker of r.menace_attackers ?? []) if (blocks.filter(([, a]) => a === attacker).length === 1) {
          const extra = r.legal_attackers?.find(h => h.attackers.includes(attacker) && !blocks.some(([b]) => b === h.blocker));
          if (extra) blocks.push([extra.blocker, attacker]); else blocks.splice(blocks.findIndex(([, a]) => a === attacker), 1);
        }
        return { response: 'blockers', blocks };
      }
      case 'damage': {
        let rest = q(r.power);
        if (r.player_recipient != null) {
          const assignments: [string, string][] = r.recipients.map(h => { const lethal = r.lethal_requirements?.find(([o]) => o === h)?.[1] ?? fail(); const amount = min(q(lethal), rest); rest -= amount; return [h, amount.toString()]; });
          return { response: 'trample_damage', assignments, player: r.player_recipient, amount: rest.toString() };
        }
        return { response: 'damage', assignments: r.recipients.map((h, i) => [h, i === 0 ? r.power : '0']) };
      }
      case 'damage_effect': { const c = first(r.candidates); return { response: 'damage_effect', effect: c.effect, damage: c.damage }; }
      case 'damage_allocation': {
        let rest = q(r.required);
        const allocations: [string, string][] = r.candidates.map(c => { const n = min(q(c.amount), rest); rest -= n; return [c.damage, n.toString()]; });
        if (rest !== 0n) return fail();
        return { response: 'damage_allocation', allocations };
      }
      default: { const unhandled: never = r; return unhandled; }
    }
  };
}
