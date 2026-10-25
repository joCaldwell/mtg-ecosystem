//! Local hosting boundary for the foundations and attachments Commander profiles.
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

/// Host code plus the engine code/toolchain/build identity. Catalogs remain
/// independently pinned by match metadata. Never use the engine-only identity
/// to authorize recovery by a potentially different host implementation.
pub static SERVER_BUILD_ID: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    mtg_contracts::sha256(
        format!(
            "mtg-server-v1\0{}\0{}",
            env!("MTG_SERVER_CODE_ID"),
            mtg_core::BUILD_ID
        )
        .as_bytes(),
    )
});
