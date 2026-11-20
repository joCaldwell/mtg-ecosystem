//! `catalog coverage` argument parsing and optional Oracle-ID selection.
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

pub const DEFAULT_OUT: &str = "generated/cards-as-data-coverage";

#[derive(Debug, PartialEq, Eq)]
pub enum Selection {
    OracleIds(PathBuf),
    DeckSnapshot(PathBuf),
}

#[derive(Debug, PartialEq, Eq)]
pub struct CoverageArgs {
    pub archive: PathBuf,
    pub out: PathBuf,
    pub selection: Option<Selection>,
}

/// Parses `ARCHIVE [--out DIR] [--only-oracle-ids FILE | --only-deck-snapshot FILE]`.
pub fn parse(args: &[String]) -> Result<CoverageArgs, String> {
    let (archive, flags) = args.split_first().ok_or("missing coverage ARCHIVE")?;
    let mut out = None;
    let mut selection = None;
    let mut flags = flags.iter();
    while let Some(flag) = flags.next() {
        let value = flags
            .next()
            .map(PathBuf::from)
            .ok_or_else(|| format!("{flag} requires a path"))?;
        match flag.as_str() {
            "--out" if out.is_none() => out = Some(value),
            "--only-oracle-ids" if selection.is_none() => {
                selection = Some(Selection::OracleIds(value))
            }
            "--only-deck-snapshot" if selection.is_none() => {
                selection = Some(Selection::DeckSnapshot(value))
            }
            _ => {
                return Err(format!(
                    "invalid or duplicate catalog coverage flag {flag:?}"
                ));
            }
        }
    }
    Ok(CoverageArgs {
        archive: archive.into(),
        out: out.unwrap_or_else(|| DEFAULT_OUT.into()),
        selection,
    })
}

impl Selection {
    pub fn load(&self) -> Result<BTreeSet<String>, String> {
        let (path, ids) = match self {
            Selection::OracleIds(path) => (path, oracle_id_lines(&read(path)?)),
            Selection::DeckSnapshot(path) => (path, snapshot_ids(&read(path)?)),
        };
        let ids = ids.map_err(|e| format!("{}: {e}", path.display()))?;
        if ids.is_empty() {
            return Err(format!("{}: selects no Oracle IDs", path.display()));
        }
        Ok(ids)
    }
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// One Oracle ID per line; blank lines and `#` comments are ignored.
fn oracle_id_lines(text: &str) -> Result<BTreeSet<String>, String> {
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect())
}

/// Distinct `decks[].cards[].oracle_id` from a Phase 1 deck snapshot.
fn snapshot_ids(text: &str) -> Result<BTreeSet<String>, String> {
    let snapshot: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let decks = snapshot["decks"].as_array().ok_or("missing decks array")?;
    let mut ids = BTreeSet::new();
    for deck in decks {
        for card in deck["cards"].as_array().ok_or("deck missing cards array")? {
            match card["oracle_id"].as_str() {
                Some(id) if !id.is_empty() => ids.insert(id.to_owned()),
                _ => return Err("card missing oracle_id".into()),
            };
        }
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn parses_legacy_and_selected_forms() {
        assert_eq!(
            parse(&args(&["a.jsonl"])).unwrap(),
            CoverageArgs {
                archive: "a.jsonl".into(),
                out: DEFAULT_OUT.into(),
                selection: None
            }
        );
        assert_eq!(
            parse(&args(&["a.jsonl", "--out", "o"])).unwrap().out,
            PathBuf::from("o")
        );
        let selected = parse(&args(&[
            "a.jsonl",
            "--only-deck-snapshot",
            "d.json",
            "--out",
            "o",
        ]))
        .unwrap();
        assert_eq!(selected.out, PathBuf::from("o"));
        assert_eq!(
            selected.selection,
            Some(Selection::DeckSnapshot("d.json".into()))
        );
        assert_eq!(
            parse(&args(&["a.jsonl", "--only-oracle-ids", "ids.txt"]))
                .unwrap()
                .selection,
            Some(Selection::OracleIds("ids.txt".into()))
        );
    }

    #[test]
    fn rejects_missing_duplicate_and_conflicting_flags() {
        for bad in [
            &[][..],
            &["a.jsonl", "--out"][..],
            &["a.jsonl", "--out", "o", "--out", "p"][..],
            &[
                "a.jsonl",
                "--only-oracle-ids",
                "i",
                "--only-deck-snapshot",
                "d",
            ][..],
            &["a.jsonl", "--bogus", "x"][..],
        ] {
            assert!(parse(&args(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn loads_distinct_ids_from_lists_and_snapshots() {
        let ids = oracle_id_lines("# comment\n b \n\na\nb\n").unwrap();
        assert_eq!(ids.into_iter().collect::<Vec<_>>(), ["a", "b"]);
        let snapshot = r#"{"decks":[{"cards":[{"oracle_id":"x"},{"oracle_id":"y"}]},
            {"cards":[{"oracle_id":"x"}]}]}"#;
        let ids = snapshot_ids(snapshot).unwrap();
        assert_eq!(ids.into_iter().collect::<Vec<_>>(), ["x", "y"]);
        assert!(snapshot_ids(r#"{"decks":[{"cards":[{"name":"z"}]}]}"#).is_err());
        assert!(snapshot_ids(r#"{"deck":[]}"#).is_err());
    }

    #[test]
    fn empty_selection_is_rejected() {
        let path = std::env::temp_dir().join(format!("coverage-ids-{}", std::process::id()));
        fs::write(&path, "# nothing\n").unwrap();
        assert!(Selection::OracleIds(path.clone()).load().is_err());
        fs::remove_file(path).unwrap();
    }
}
