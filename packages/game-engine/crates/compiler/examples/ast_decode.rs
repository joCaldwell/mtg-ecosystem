//! Diagnostic: decode every archived face result (and R4 resolution) through
//! the generated typed AST and print each distinct failure.
use std::collections::BTreeMap;
use std::io::BufRead;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("usage: ast_decode ARCHIVE")?;
    let file = std::io::BufReader::new(std::fs::File::open(path)?);
    let (mut faces, mut resolutions) = (0usize, 0usize);
    let mut failures: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for line in file.lines().skip(1) {
        let line = line?;
        let record: serde_json::Value = serde_json::from_str(&line)?;
        let name = record["source"]["name"].as_str().unwrap_or("").to_owned();
        for face in record["faces"].as_array().into_iter().flatten() {
            if let Some(result) = face.get("result") {
                faces += 1;
                if let Err(e) =
                    serde_json::from_value::<mtg_compiler::ast::ParseCardResult>(result.clone())
                {
                    let entry = failures.entry(e.to_string()).or_insert((0, name.clone()));
                    entry.0 += 1;
                }
            }
        }
        if let Some(resolution) = record.get("resolution") {
            resolutions += 1;
            if let Err(e) =
                serde_json::from_value::<mtg_compiler::ast::CardResolution>(resolution.clone())
            {
                let entry = failures
                    .entry(format!("resolution: {e}"))
                    .or_insert((0, name.clone()));
                entry.0 += 1;
            }
        }
    }
    println!(
        "faces {faces} resolutions {resolutions} distinct failures {}",
        failures.len()
    );
    for (error, (count, example)) in &failures {
        println!("{count}\t{example}\t{error}");
    }
    Ok(())
}
