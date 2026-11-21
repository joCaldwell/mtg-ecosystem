// Engine build identity at the executable edge; see crates/build_identity.rs.
#[path = "../build_identity.rs"]
mod build_identity;

fn main() {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    build_identity::emit(&manifest.join("../../../.."));
}
