//! Local hosting boundary for the Foundations Commander profile.
//! Protocol code is not evidence that HTTP hosting/recovery has been completed.
pub mod protocol;
pub mod store;

pub mod auth;
pub mod worker;

pub mod broker;
pub mod ipc;
pub mod network;
pub mod process;

pub mod http;

pub mod codec;
pub mod registry;

mod recovery_hashes;
mod resume_reader;

/// Engine build identity of this executable, computed by build.rs from the
/// production engine source closure (`crates/build_identity.rs`). `main`
/// installs it into mtg-core before any engine use; host metadata, admission
/// and recovery bind this value. Tests leave core at `BuildIdentity::TEST` and
/// never compare a core-stamped identity with this one.
pub const ENGINE_BUILD: mtg_core::BuildIdentity = mtg_core::compiled_build_identity!();

/// Host code plus the engine code/toolchain/build identity. Catalogs remain
/// independently pinned by match metadata. Never use the engine-only identity
/// to authorize recovery by a potentially different host implementation.
pub static SERVER_BUILD_ID: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    mtg_contracts::sha256(
        format!(
            "mtg-server-v1\0{}\0{}",
            env!("MTG_SERVER_CODE_ID"),
            crate::ENGINE_BUILD.id
        )
        .as_bytes(),
    )
});

pub mod delivery;

pub mod admission;

pub mod lifecycle;
