//! CR 106.7, 106.1a and 305.6: potential output includes every land
//! ability's colored output, without paying its costs.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl RulesTxn {
    pub(crate) fn opponent_land_mana_colors(&self, player: PlayerId) -> Result<Vec<Color>, String> {
        if !self.is_live(player) {
            return Err("opponent-land mana requires a live activator".into());
        }
        let mut output: [BTreeSet<Color>; 4] = std::array::from_fn(|_| BTreeSet::new());
        let mut has_dynamic_land = [false; 4];
        for object in self.state.rules.objects.values() {
            // Event and departure boundaries reconcile layer-2 control before
            // payment or queries. Frozen losing owners are virtually absent.
            if !object.present_on_battlefield()
                || !self.is_live(object.controller)
                || !self.is_live(self.state.rules.cards[&object.id.card].owner)
            {
                continue;
            }
            let definition = self.copiable_definition(object.id)?;
            let mut characteristics = definition.characteristics.clone();
            self.apply_land_subtype_grants(object.id, &mut characteristics)?;
            if !characteristics.has_type(CardKind::Land) {
                continue;
            }
            let seat = usize::from(object.controller.0);
            output[seat].extend(
                characteristics
                    .subtypes
                    .iter()
                    .filter_map(|subtype| basic_land_color(subtype)),
            );
            if self.has_granted_land_mana(object.id)? {
                output[seat].extend(Self::granted_land_mana_colors());
            }
            for ability in &definition.mana_abilities {
                if ability.production == ManaProduction::OpponentLandColors {
                    has_dynamic_land[seat] = true;
                } else {
                    output[seat].extend(
                        self.mana_colors(object.controller, &ability.production)?
                            .into_iter()
                            .filter(|color| *color != Color::C),
                    );
                }
            }
        }

        // Each dynamic land sees precisely the output sets of the other seats.
        // Start with only grounded output. Each changing pass adds at least
        // one of the twenty seat/color pairs, so empty cycles stay empty.
        loop {
            let mut next = output.clone();
            for (seat, colors) in next.iter_mut().enumerate() {
                if has_dynamic_land[seat] {
                    colors.extend(opponent_colors(&output, seat));
                }
            }
            if next == output {
                return Ok(opponent_colors(&output, usize::from(player.0))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect());
            }
            output = next;
        }
    }
}

fn opponent_colors(output: &[BTreeSet<Color>; 4], seat: usize) -> impl Iterator<Item = Color> + '_ {
    output
        .iter()
        .enumerate()
        .filter(move |(controller, _)| *controller != seat)
        .flat_map(|(_, colors)| colors.iter().copied())
}
