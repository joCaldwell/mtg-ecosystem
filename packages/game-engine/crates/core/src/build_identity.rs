//! Executable build identity. mtg-core never reads build-time environment: the
//! executable edge (mtg-sim, mtg-server) computes the identity from production
//! engine sources in its build script (`crates/build_identity.rs`) and installs
//! it once at startup. Every snapshot, saved work and search artifact this
//! process stamps or admits is fenced by the installed identity.
use std::sync::OnceLock;

/// Engine code/toolchain identity of one executable build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildIdentity {
    /// SHA-256 of the production engine source closure plus toolchain metadata.
    pub id: &'static str,
    pub profile: &'static str,
    pub opt_level: &'static str,
    pub debug: &'static str,
    pub debug_assertions: &'static str,
    pub target: &'static str,
    pub rustflags: &'static str,
    pub rustc: &'static str,
}

impl BuildIdentity {
    /// Identity of a process that installed none: unit/integration tests and
    /// examples. It is not a SHA-256, so it can never collide with a production
    /// build, and artifacts stamped with it only restore in such processes.
    pub const TEST: Self = Self {
        id: "test-build-identity-v1",
        profile: "test",
        opt_level: "test",
        debug: "test",
        debug_assertions: if cfg!(debug_assertions) {
            "true"
        } else {
            "false"
        },
        target: "test",
        rustflags: "",
        rustc: "test",
    };
}

/// The identity a build script that runs `crates/build_identity.rs` emitted for
/// the invoking crate. `env!` expands in that crate, never in mtg-core.
#[macro_export]
macro_rules! compiled_build_identity {
    () => {
        $crate::BuildIdentity {
            id: env!("MTG_BUILD_ID"),
            profile: env!("MTG_PROFILE"),
            opt_level: env!("MTG_OPT_LEVEL"),
            debug: env!("MTG_DEBUG"),
            debug_assertions: env!("MTG_DEBUG_ASSERTIONS"),
            target: env!("MTG_TARGET"),
            rustflags: env!("MTG_CARGO_ENCODED_RUSTFLAGS"),
            rustc: env!("MTG_RUSTC"),
        }
    };
}

static ACTIVE: OnceLock<BuildIdentity> = OnceLock::new();

/// Installs this process's identity before any engine use. Idempotent for the
/// same value; any other value, or installing after the identity was already
/// read (which fixes it to [`BuildIdentity::TEST`]), is an error.
pub fn install_build_identity(identity: BuildIdentity) -> Result<(), String> {
    let active = ACTIVE.get_or_init(|| identity);
    if *active == identity {
        Ok(())
    } else {
        Err(format!(
            "engine build identity already fixed to {}; cannot install {}",
            active.id, identity.id
        ))
    }
}

/// The installed identity, or [`BuildIdentity::TEST`] once read without one.
pub fn build_identity() -> &'static BuildIdentity {
    ACTIVE.get_or_init(|| BuildIdentity::TEST)
}

/// Shorthand for `build_identity().id`.
pub fn build_id() -> &'static str {
    build_identity().id
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_identity_records_the_target_debug_assertions() {
        assert_eq!(
            BuildIdentity::TEST.debug_assertions,
            cfg!(debug_assertions).to_string()
        );
        assert_ne!(BuildIdentity::TEST.id.len(), 64);
    }
    #[test]
    fn an_uninstalled_process_is_fixed_to_the_test_identity() {
        assert_eq!(build_id(), BuildIdentity::TEST.id);
        assert!(install_build_identity(BuildIdentity::TEST).is_ok());
        let other = BuildIdentity {
            id: "0000000000000000000000000000000000000000000000000000000000000000",
            ..BuildIdentity::TEST
        };
        assert!(install_build_identity(other).is_err());
        assert_eq!(build_id(), BuildIdentity::TEST.id);
    }
}
