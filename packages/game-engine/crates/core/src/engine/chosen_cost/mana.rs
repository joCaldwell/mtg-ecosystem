//! CR 118.3, 601.2g-h, 602.2b: preflight the mana and chosen taps together.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

// A priority offer is a bounded query. A large set of alternative mana
// allocations can exhaust this proof, so only offer a payment we can prove.
const MAX_FUNDING_STATES: usize = 4096;

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
struct ManaShortfall {
    total: Quantity,
    colored: [Quantity; 6],
}

impl ManaShortfall {
    fn new(cost: &ManaCost, pool: &BTreeMap<Color, Quantity>) -> Self {
        let one = Quantity::from(1);
        let mut shortfall = Self {
            total: cost.generic.clone(),
            colored: std::array::from_fn(|_| Quantity::zero()),
        };
        for &color in &cost.colored {
            shortfall.total.add(&one);
            shortfall.colored[color as usize].add(&one);
        }
        for (&color, amount) in pool {
            shortfall.credit(color, amount);
        }
        shortfall
    }

    fn credit(&mut self, color: Color, amount: &Quantity) {
        let colored = &mut self.colored[color as usize];
        self.total
            .subtract(&std::cmp::min(self.total.clone(), amount.clone()));
        colored.subtract(&std::cmp::min(colored.clone(), amount.clone()));
    }

    fn paid(&self) -> bool {
        self.total == Quantity::zero() && self.colored.iter().all(|n| *n == Quantity::zero())
    }
}

impl RulesTxn {
    pub(super) fn can_fund_chosen_cost(
        &self,
        player: PlayerId,
        source: ObjectId,
        cost: &ChosenObjectCost,
        mana: &ManaCost,
        eligible: &[ObjectId],
        count: usize,
    ) -> Result<bool, String> {
        let initial = ManaShortfall::new(mana, &self.state.rules.players[player.0 as usize].mana);
        if initial.paid() {
            return Ok(true);
        }
        let can_spend = eligible.len() - count;
        let mut plans = BTreeMap::from([(initial, 0usize)]);
        for object in self.state.rules.objects.values() {
            if !object.present_on_battlefield()
                || object.controller != player
                || object.tapped
                || (cost.tap_source && object.id == source)
            {
                continue;
            }
            let takes_candidate =
                usize::from(cost.action == Tap && eligible.binary_search(&object.id).is_ok());
            if takes_candidate > can_spend {
                continue;
            }
            let production = self.chosen_funding_options(player, object.id)?;
            if production.is_empty() {
                continue;
            }
            let mut next = plans.clone();
            for (shortfall, spent) in plans {
                let spent = spent + takes_candidate;
                if spent > can_spend {
                    continue;
                }
                // This permanent can supply one of its mana abilities.
                // Identical shortfalls keep the plan using fewer tap candidates.
                for output in &production {
                    let mut remaining = shortfall.clone();
                    for (color, amount) in output {
                        remaining.credit(*color, amount);
                    }
                    if remaining.paid() {
                        return Ok(true);
                    }
                    next.entry(remaining)
                        .and_modify(|n| *n = (*n).min(spent))
                        .or_insert(spent);
                    if next.len() > MAX_FUNDING_STATES {
                        return Ok(false);
                    }
                }
            }
            plans = next;
        }
        Ok(false)
    }

    fn chosen_funding_options(
        &self,
        player: PlayerId,
        source: ObjectId,
    ) -> Result<Vec<Vec<(Color, Quantity)>>, String> {
        let Ok(choices) = self.mixed_mana_choices(player, source) else {
            return Ok(vec![]);
        };
        let current = self.current_mana_abilities(source)?;
        let one = Quantity::from(1);
        let mut options = vec![];
        for choice in choices {
            match choice {
                ManaAbilityIdentity::Intrinsic { color } => {
                    options.push(vec![(color, one.clone())])
                }
                ManaAbilityIdentity::Printed { .. } | ManaAbilityIdentity::Granted { .. } => {
                    let Some((_, ability)) =
                        current.iter().find(|(slot, _)| slot.identity() == choice)
                    else {
                        return Err("missing mana ability choice".into());
                    };
                    // A conservative preflight can combine independent mana sources.
                    // Inputs, life and riders would require simulating their order.
                    if ability.spend_restriction.is_some()
                        || !ability.costs.tap
                        || ability.costs.mana.is_some()
                        || ability.costs.pay_life.is_some()
                        || !ability.riders.is_empty()
                        || ability.costs.remove_source_plus_one_counters.is_some()
                    {
                        continue;
                    }
                    match &ability.production {
                        ManaProduction::Fixed { symbols } => options
                            .push(symbols.iter().map(|&color| (color, one.clone())).collect()),
                        ManaProduction::Bundles { options: bundles } => options.extend(
                            bundles
                                .iter()
                                .map(|symbols| symbols.iter().map(|&c| (c, one.clone())).collect()),
                        ),
                        ManaProduction::Choose { colors, amount } => options
                            .extend(colors.iter().map(|&color| vec![(color, amount.clone())])),
                        production => options.extend(
                            self.mana_colors(player, production)?
                                .into_iter()
                                .map(|color| vec![(color, one.clone())]),
                        ),
                    }
                }
            }
        }
        Ok(options)
    }
}
