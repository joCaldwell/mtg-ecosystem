//! Engine build-identity file selection. Dependency-free so the mtg-sim and
//! mtg-server build scripts include this exact file (`#[path]`) while the native
//! source-closure verifier calls it through `mtg_contracts::engine_build_input`.

/// Engine build identity covers production engine source only. It excludes
/// card/profile data, hosting code and test-only sources (`tests/`,
/// `examples/`, `benches/`, `tests.rs`, `*_tests.rs`, `*_tests/`, the
/// `testkit` harness module and the dev-only `crates/witness` package), which
/// never enter a production executable: every test-named module is
/// `#[cfg(test)]`, every `testkit` module is `#[cfg(feature = "testkit")]` and
/// only test crates enable that feature, all enforced by
/// `scripts/engine-structure-check.py`. Retained evidence still carries those
/// files with their own hashes; matches/snapshots separately bind the exact
/// catalog digest.
pub fn engine_build_input(path: &str) -> bool {
    matches!(path, "Cargo.toml" | "Cargo.lock" | "rust-toolchain.toml")
        || (path.starts_with("packages/game-engine/crates/")
            && !path.starts_with("packages/game-engine/crates/witness/")
            && !path.split('/').any(|p| {
                matches!(
                    p,
                    "generated"
                        | "target"
                        | "node_modules"
                        | ".git"
                        | "tests"
                        | "examples"
                        | "benches"
                )
            })
            && (path.ends_with("/Cargo.toml")
                || (path.ends_with(".rs") && !test_only_source(path))))
}

/// Test-only module naming: a `tests`/`*_tests`/`testkit` file or directory
/// component.
fn test_only_source(path: &str) -> bool {
    path.split('/').any(|part| {
        let stem = part.strip_suffix(".rs").unwrap_or(part);
        stem == "tests" || stem.ends_with("_tests") || stem == "testkit"
    })
}

#[cfg(test)]
mod tests {
    use super::engine_build_input;
    #[test]
    fn build_identity_covers_production_engine_source_only() {
        for path in [
            "Cargo.lock",
            "Cargo.toml",
            "rust-toolchain.toml",
            "packages/game-engine/crates/sim/src/new_module.rs",
            "packages/game-engine/crates/core/build.rs",
            "packages/game-engine/crates/build_identity.rs",
            "packages/game-engine/crates/compiler/Cargo.toml",
            "packages/game-engine/crates/core/src/engine/testing.rs",
            "packages/game-engine/crates/core/src/engine/attests.rs",
        ] {
            assert!(engine_build_input(path), "{path}");
        }
        for path in [
            "packages/game-engine/tests/fixtures/catalogs/base.json",
            "packages/game-server/src/main.rs",
            "packages/game-engine/crates/core/generated/export.rs",
            "packages/game-engine/crates/core/README.md",
            "packages/game-engine/crates/core/src/engine/tokens_tests.rs",
            "packages/game-engine/crates/core/src/engine/search/tests.rs",
            "packages/game-engine/crates/core/src/engine/definition_diversity_tests/heterogeneous.rs",
            "packages/game-engine/crates/core/tests/rules/combat_adversarial.rs",
            "packages/game-engine/crates/core/tests/support/mod.rs",
            "packages/game-engine/crates/core/examples/action_query_schema.rs",
            "packages/game-engine/crates/compiler/examples/coverage.rs",
            "packages/game-engine/crates/sim/src/policy/generic_policy_tests.rs",
            "packages/game-engine/crates/core/src/testkit.rs",
            "packages/game-engine/crates/core/src/testkit/driver.rs",
            "packages/game-engine/crates/witness/Cargo.toml",
            "packages/game-engine/crates/witness/src/lib.rs",
            "packages/game-engine/crates/witness/src/equipped.rs",
            "packages/game-engine/crates/witness/tests/cards/main.rs",
        ] {
            assert!(!engine_build_input(path), "{path}");
        }
    }
}
