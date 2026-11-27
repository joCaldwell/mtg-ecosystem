//! Direct decisions for each lowered definition in an archive comparison.
use mtg_contracts::{
    Catalog,
    kir::{ExecutionTier, definition_tiers},
};
use std::{env, fs::File, io, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = env::args_os().nth(1).ok_or("supply bulk catalog.json")?;
    let catalog: Catalog =
        serde_json::from_reader(io::BufReader::new(File::open(Path::new(&file))?))?;
    catalog.validate()?;
    let tiers = catalog
        .definitions
        .iter()
        .map(|definition| {
            let tiers = definition_tiers(definition)?;
            Ok((
                &definition.oracle_id,
                [
                    ExecutionTier::Executes,
                    ExecutionTier::HeadlessQualified,
                    ExecutionTier::Search,
                    ExecutionTier::Hosting,
                ]
                .map(|tier| tiers.admits(tier)),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    serde_json::to_writer(io::stdout().lock(), &tiers)?;
    Ok(())
}
