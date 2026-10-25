//! Compiler-only entry point for repeated intake measurements.
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err(
            "usage: cargo run --release -p mtg-compiler --example coverage -- ARCHIVE OUTPUT"
                .into(),
        );
    }
    let report = mtg_compiler::coverage::coverage(Path::new(&args[0]), Path::new(&args[1]))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
