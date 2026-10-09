//! Keeping Roblox on its public (LIVE) release channel.
//!
//! Roblox tries new builds on a few accounts first by putting them on a
//! rollout channel ("zcanary"…). A client on one checks its build against
//! that channel's, decides it's out of date and runs Roblox's installer.
//! The client reads its channel from the registry, where Roblox's installer
//! (and Bloxstrap) keep it, so LIVE is written there before each launch.

use super::compat::registry;

const KEY: &str = r"Software\ROBLOX Corporation\Environments\RobloxPlayer\Channel";
const VALUE: &str = "www.roblox.com";
/// What Roblox's installer writes for LIVE.
const LIVE: &str = "production";

/// Puts the client on LIVE for its next start.
pub fn force_live() {
    if registry::get(KEY, VALUE).as_deref() != Some(LIVE) {
        registry::set(KEY, VALUE, LIVE);
    }
}
