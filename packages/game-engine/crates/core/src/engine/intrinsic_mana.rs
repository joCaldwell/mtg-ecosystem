//! CR 305.6, 602.2, 605.3: choose WHICH intrinsic ability before activation.
//! A paid explicit ability's later production-color choice is a separate stage.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
impl Game {
    pub(super) fn intrinsic_mana_colors(&self, source: ObjectId) -> Result<Vec<Color>, String> {
        let object = self.current(source)?;
        if object.zone != Zone::Battlefield
            || !self
                .definition(source.card)?
                .characteristics
                .has_type(CardKind::Land)
        {
            return Ok(vec![]);
        }
        let c = self.characteristics(source)?;
        Ok(c.subtypes
            .iter()
            .filter_map(|s| basic_land_color(s))
            .collect())
    }
    pub(super) fn intrinsic_choice_colors(
        &self,
        player: PlayerId,
        source: ObjectId,
    ) -> Result<Vec<Color>, String> {
        self.validate_simple_tap_mana(player, source)?;
        let colors = self.intrinsic_mana_colors(source)?;
        if colors.is_empty() {
            return Err("missing unpaid intrinsic mana colors".into());
        }
        Ok(colors)
    }

    pub(super) fn validate_simple_tap_mana(
        &self,
        player: PlayerId,
        source: ObjectId,
    ) -> Result<(), String> {
        let o = self.current(source)?;
        if !self.is_live(player)
            || o.zone != Zone::Battlefield
            || o.controller != player
            || o.tapped
            || (self.characteristics(source)?.has_type(CardKind::Creature)
                && !o.attack_ready
                && !self.has_keyword(source, KeywordAbility::Haste))
        {
            return Err("invalid unpaid tap mana source".into());
        }
        Ok(())
    }
    pub(super) fn validate_intrinsic_choice(
        &self,
        player: PlayerId,
        source: ObjectId,
        color: Color,
    ) -> Result<(), String> {
        if !self
            .intrinsic_choice_colors(player, source)?
            .contains(&color)
        {
            return Err("invalid intrinsic mana color".into());
        }
        Ok(())
    }
    pub(super) fn mana_resume_player(&self, resume: &Frame, player: PlayerId) -> bool {
        match resume {
            Frame::Priority => self.state.rules.priority == player,
            Frame::ResolutionPayment { payment, .. } => {
                payment.stage == ResolutionPaymentStage::Mana && payment.payer == player
            }
            Frame::Mana { .. } => self.top().is_ok_and(|s| s.controller == player),
            Frame::ManaAbility { activation } => {
                activation.player == player
                    && activation.stage == mana_permanents::ManaStage::Payment
            }
            _ => false,
        }
    }
    pub(super) fn begin_intrinsic_mana(
        &mut self,
        input: &AcceptedInput,
        source: ObjectId,
        resume: Frame,
    ) -> Result<(), String> {
        self.validate_mana_activation(input.player, source)?;
        let colors = self.intrinsic_choice_colors(input.player, source)?;
        if !self.mana_resume_player(&resume, input.player) {
            return Err("invalid intrinsic mana continuation".into());
        }
        if let [color] = colors.as_slice() {
            let receipt = self.activate_selected_intrinsic_mana(input, source, *color)?;
            self.resume_mana(resume, vec![receipt])
        } else {
            self.state.rules.frame = Frame::ManaSourceChoice {
                player: input.player,
                source,
                scope: ManaSourceScope::Intrinsic,
                resume: Box::new(resume),
            };
            Ok(())
        }
    }
}
