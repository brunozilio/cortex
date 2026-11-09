//! Superfik updates itself; it is never published in an app store.
//!
//! Each GitHub release carries a manifest per platform
//! (`releases/latest/download/superfik-<platform>.json`, `Releases`);
//! `offer` says whether it is newer than the running version and whether
//! it is required (below the release's minimum). `Updater::prepare`
//! downloads the archive (`Fetch`), checks its size, SHA-256 and the
//! Ed25519 signature over that digest with `UPDATE_KEY`, and stages it
//! beside the install (`Installer`: on macOS also the app's code
//! signature and the publisher's team); `restart`
//! starts a detached swap that replaces the install once the app has quit
//! and relaunches it. `installer()` is `None` where the install cannot
//! replace itself (a `cargo run` binary, a `.deb`, Windows for now).

mod error;
mod http;
mod install;
mod manifest;
mod updater;
mod verify;
mod version;

pub use error::{Stage, UpdateError};
pub use http::{Fetch, HttpFetch, HttpReleases, Releases, SERVICE};
pub use install::{
    Commands, Installer, Layout, MacApp, Prepared, Ran, SWAP_SCRIPT, SwapInstaller, System, TEAM_ID, Tarball,
    app_bundle_of, installer, short_version, tarball_root_of, team_of, writable,
};
pub use manifest::{Asset, Manifest, Offer, offer};
pub use updater::Updater;
pub use verify::{UPDATE_KEY, digest_file, verify};
pub use version::{BadVersion, Platform, Version};

#[cfg(test)]
mod tests_install;
#[cfg(test)]
mod tests_updater;
