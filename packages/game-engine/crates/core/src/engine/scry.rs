//! CR 109.5, 400.7, 608.2c/h, 701.22a-d. Looking and same-zone ordering
//! never create successor incarnations or a priority window.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl RulesTxn {
    pub(crate) fn library_prefix(
        &self,
        player: PlayerId,
        amount: &Quantity,
    ) -> Result<Vec<ObjectId>, String> {
        if amount.is_negative() {
            return Err("negative scry amount".into());
        }
        let mut looked = vec![];
        for (index, card) in self
            .state
            .rules
            .libraries
            .get(&player)
            .into_iter()
            .flatten()
            .enumerate()
        {
            if Quantity::from(i64::try_from(index).map_err(|_| "scry index limit")?) >= *amount {
                break;
            }
            let object = self.object_id(*card).ok_or("missing scry library card")?;
            if self.current(object)?.zone != (Zone::Library { owner: player }) {
                return Err("scry library zone invariant".into());
            }
            looked.push(object);
        }
        Ok(looked)
    }

    pub(super) fn validate_library_partition(
        &self,
        player: PlayerId,
        looked: &[ObjectId],
        top: &[ObjectId],
        bottom: &[ObjectId],
    ) -> Result<(), String> {
        if !self.is_live(player)
            || top.len().checked_add(bottom.len()) != Some(looked.len())
            || top.iter().chain(bottom).copied().collect::<BTreeSet<_>>()
                != looked.iter().copied().collect::<BTreeSet<_>>()
            || looked.iter().collect::<BTreeSet<_>>().len() != looked.len()
        {
            return Err("invalid scry partition".into());
        }
        Ok(())
    }

    pub(super) fn reorder_library_prefix(
        &mut self,
        player: PlayerId,
        looked: &[ObjectId],
        top: &[ObjectId],
        bottom: &[ObjectId],
    ) -> Result<(), String> {
        self.validate_library_partition(player, looked, top, bottom)?;
        if let Some(library) = self.rules_mut().libraries.get_mut(&player) {
            *library = top
                .iter()
                .map(|o| o.card)
                .chain(library[looked.len()..].iter().copied())
                .chain(bottom.iter().map(|o| o.card))
                .collect();
        }
        Ok(())
    }
}
