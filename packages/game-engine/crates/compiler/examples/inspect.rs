//! Diagnostic: print compile results of named archive records.
//!
//! `inspect ARCHIVE 'Exact Name'...` prints one line per record with its
//! Oracle text and compile result (provenance elided for readability).
//! `inspect --catalog ARCHIVE 'Exact Name'...` prints one executable catalog
//! of the records that lowered, for schema validation.
//! `inspect --schema` prints the executable catalog JSON Schema.
use mtg_compiler::CompileResult;
use std::io::{BufRead, BufReader};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--schema"] {
        println!(
            "{}",
            serde_json::to_string(&mtg_contracts::catalog_schema())?
        );
        return Ok(());
    }
    let catalog = args.first().is_some_and(|a| a == "--catalog");
    if catalog {
        args.remove(0);
    }
    let [archive, names @ ..] = args.as_slice() else {
        return Err("usage: inspect [--catalog] ARCHIVE NAME... | inspect --schema".into());
    };
    let mut lines = BufReader::new(std::fs::File::open(archive)?).lines();
    let manifest = lines.next().ok_or("empty archive")??;
    let mut definitions = vec![];
    for line in lines {
        let line = line?;
        if !names.iter().any(|name| {
            line.contains(&format!(
                "\"name\":{}",
                serde_json::to_string(name).unwrap()
            ))
        }) {
            continue;
        }
        let row: serde_json::Value = serde_json::from_str(&line)?;
        if !names.iter().any(|name| row["source"]["name"] == **name) {
            continue;
        }
        let result = mtg_compiler::compile(&manifest, &line);
        if catalog {
            if let CompileResult::Lowered { definition } = result {
                definitions.push(*definition);
            }
            continue;
        }
        let mut result = serde_json::to_value(result)?;
        if let Some(provenance) = result["definition"].get_mut("provenance") {
            *provenance = serde_json::Value::Null;
        }
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "name": row["source"]["name"],
                "oracle_text": row["source"]["oracle_text"],
                "result": result,
            }))?
        );
    }
    if catalog {
        definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        let catalog = mtg_contracts::Catalog {
            schema_version: mtg_contracts::CURRENT_IR_VERSION,
            rules_edition: mtg_contracts::RULES_EDITION.into(),
            rules_sha256: mtg_contracts::RULES_SHA256.into(),
            definitions,
        };
        catalog.validate()?;
        println!("{}", serde_json::to_string(&catalog)?);
    }
    Ok(())
}
