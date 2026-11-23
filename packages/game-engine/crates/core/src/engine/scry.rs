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
}
