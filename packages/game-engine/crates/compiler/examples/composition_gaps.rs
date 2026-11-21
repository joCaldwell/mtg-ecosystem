//! Diagnostic: rank what the compositional pipeline still lacks.
//! Prints `oracle_id<TAB>name<TAB>commander_legal<TAB>reason` for every
//! record that neither a family nor block-by-block composition lowers.
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        return Err(
            "usage: cargo run -p mtg-compiler --example composition_gaps -- ARCHIVE".into(),
        );
    }
    println!("oracle_id\tname\tcommander_legal\treason");
    for (id, name, legal, reason) in mtg_compiler::coverage::composition_gaps(Path::new(&args[0]))?
    {
        let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
        println!(
            "{}\t{}\t{legal}\t{}",
            clean(&id),
            clean(&name),
            clean(&reason)
        );
    }
    Ok(())
}
