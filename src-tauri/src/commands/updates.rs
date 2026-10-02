//! Updates.
//!
//! Releases are published on GitHub with a signed `latest.json`. Clide checks
//! it at most once a day, tells the user when a newer version exists, and
//! installs it only when they press the button.
//!
//! The package is verified against the public key compiled into the app
//! (`plugins.updater.pubkey` in `tauri.conf.json`), so a download that was not
//! signed with Clide's private key is rejected no matter where it came from.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

use crate::database::{kv, now_ms};
use crate::state::AppState;

const CACHE_KEY: &str = "updates.latest_release";
const CHECK_INTERVAL_MS: i64 = 24 * 60 * 60 * 1_000;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CachedRelease {
    version: String,
    checked_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    current_version: String,
    latest_version: Option<String>,
    update_available: bool,
    checked_at: Option<i64>,
}

fn normalize_version(value: &str) -> &str {
    value
        .trim()
        .strip_prefix('v')
        .or_else(|| value.trim().strip_prefix('V'))
        .unwrap_or(value.trim())
}

fn is_newer(current: &str, latest: &str) -> bool {
    let Ok(current) = semver::Version::parse(normalize_version(current)) else {
        return false;
    };
    let Ok(latest) = semver::Version::parse(normalize_version(latest)) else {
        return false;
    };
    latest > current
}

fn status(current: &str, cached: Option<&CachedRelease>) -> UpdateStatus {
    let latest_version = cached.map(|release| normalize_version(&release.version).to_string());
    UpdateStatus {
        current_version: current.to_string(),
        update_available: latest_version
            .as_deref()
            .map(|latest| is_newer(current, latest))
            .unwrap_or(false),
        latest_version,
        checked_at: cached.map(|release| release.checked_at),
    }
}

/// Check at most once per 24 hours unless the user presses Check now.
#[tauri::command]
pub async fn check_for_updates(app: AppHandle, force: bool) -> Result<UpdateStatus, String> {
    let state = app.state::<AppState>();
    let current = env!("CARGO_PKG_VERSION");
    let cached =
        kv::get::<CachedRelease>(&state.db.lock(), CACHE_KEY).map_err(|error| error.to_string())?;

    if !force
        && cached
            .as_ref()
            .is_some_and(|release| now_ms().saturating_sub(release.checked_at) < CHECK_INTERVAL_MS)
    {
        return Ok(status(current, cached.as_ref()));
    }

    let found = app
        .updater()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| format!("Could not check for updates: {error}"))?;

    // No newer release is the same as "latest is what I already have".
    let release = CachedRelease {
        version: found
            .map(|update| update.version)
            .unwrap_or_else(|| current.to_string()),
        checked_at: now_ms(),
    };
    kv::set(&state.db.lock(), CACHE_KEY, &release).map_err(|error| error.to_string())?;

    Ok(status(current, Some(&release)))
}

/// Download the newer version, verify it, install it, and relaunch.
///
/// Only ever called from the button in Settings: Clide never replaces itself
/// without being asked.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let update = app
        .updater()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| format!("Could not check for updates: {error}"))?
        .ok_or_else(|| "clide is already up to date.".to_string())?;

    tracing::info!(version = %update.version, "installing update");

    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|error| format!("The update could not be installed: {error}"))?;

    app.restart()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison_handles_release_tags() {
        assert!(is_newer("0.1.0", "v0.1.1"));
        assert!(is_newer("1.9.9", "2.0.0"));
        assert!(!is_newer("0.1.1", "v0.1.1"));
        assert!(!is_newer("0.2.0", "v0.1.9"));
    }

    #[test]
    fn cached_release_becomes_a_frontend_status() {
        let cached = CachedRelease {
            version: "v0.2.0".into(),
            checked_at: 42,
        };
        let result = status("0.1.0", Some(&cached));
        assert!(result.update_available);
        assert_eq!(result.latest_version.as_deref(), Some("0.2.0"));
        assert_eq!(result.checked_at, Some(42));
    }

    #[test]
    fn the_current_version_is_never_an_update() {
        let cached = CachedRelease {
            version: "2.0.0".into(),
            checked_at: 1,
        };
        assert!(!status("2.0.0", Some(&cached)).update_available);
    }
}
