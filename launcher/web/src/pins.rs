//! The pinned Minecraft version, shared with the desktop launcher (launcher/src/sources.rs's
//! `Pins`): baked from research-lock.json at build time. The repository and version of the
//! build are baked the same way the desktop launcher's backend bakes them.

/// The repository whose releases this web launcher offers (set at build time by the fork's
/// workflow; Horizon's own repository otherwise), as `backend::REPO` is for the launcher.
pub const REPO: &str = match option_env!("HORIZON_GAME_REPO") {
    Some(repo) => repo,
    None => "horizon-rust/horizon",
};

/// The launcher version this web build belongs to ("dev" outside a release build).
pub const VERSION: &str = match option_env!("HORIZON_LAUNCHER_VERSION") {
    Some(version) => version,
    None => "dev",
};

include!(concat!(env!("OUT_DIR"), "/pins.rs"));
