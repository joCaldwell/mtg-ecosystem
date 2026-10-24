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
