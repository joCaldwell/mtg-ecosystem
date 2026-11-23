//! Diagnostic: rank what the compositional pipeline still lacks.
//! Prints `oracle_id<TAB>name<TAB>commander_legal<TAB>reason<TAB>binding<TAB>block`
//! for every record block-by-block composition does not lower. `reason` is the first blocker once card-wide binding checks are
//! deferred; `binding` is that card-wide rejection (`-` when the graph passes)
//! and `block` the printed block `reason` names (`-` when not per-block).
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        return Err(
            "usage: cargo run -p mtg-compiler --example composition_gaps -- ARCHIVE".into(),
        );
    }
    println!("oracle_id\tname\tcommander_legal\treason\tbinding\tblock");
    for gap in mtg_compiler::coverage::composition_gaps(Path::new(&args[0]))? {
        let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            clean(&gap.oracle_id),
            clean(&gap.name),
            gap.commander_legal,
            clean(&gap.reason),
            gap.binding.as_deref().map_or("-".into(), clean),
            gap.block.map_or("-".into(), |b| b.to_string()),
        );
    }
    Ok(())
}
