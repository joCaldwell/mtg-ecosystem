//! Diagnostic: rank what the compositional pipeline still lacks.
//! Prints `oracle_id<TAB>name<TAB>commander_legal<TAB>kind<TAB>block<TAB>code`,
//! one line per blocker of every record block-by-block composition does not
//! lower: each card-wide binding failure (`binding`), the first failure of
//! each printed block (`block`, with its index) and any card-wide failure
//! outside the blocks (`card`). `block` is `-` when not per-block.
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        return Err(
            "usage: cargo run -p mtg-compiler --example composition_gaps -- ARCHIVE".into(),
        );
    }
    println!("oracle_id\tname\tcommander_legal\tkind\tblock\tcode");
    for gap in mtg_compiler::coverage::composition_gaps(Path::new(&args[0]))? {
        let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
        for blocker in &gap.blockers {
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}",
                clean(&gap.oracle_id),
                clean(&gap.name),
                gap.commander_legal,
                blocker.kind.as_str(),
                blocker.block.map_or("-".into(), |b| b.to_string()),
                clean(&blocker.code),
            );
        }
    }
    Ok(())
}
