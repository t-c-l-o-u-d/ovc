// GNU Affero General Public License v3.0 or later (see LICENSE or https://www.gnu.org/licenses/agpl.txt)
//! Cache management for OpenShift client versions
//!
//! This module handles caching of version information with download URLs for all platforms
//! to minimize API calls to the OpenShift mirror. The cache expires after 72 hours and is
//! also updated when requested versions are not found.

use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::{Platform, compare_versions};

/// Cache time-to-live: 72 hours in seconds
const CACHE_TTL_SECS: u64 = 72 * 60 * 60;

/// Version information with download URLs for all platforms
#[derive(Serialize, Deserialize, Clone)]
pub struct VersionInfo {
    /// Version string (e.g. "4.19.0")
    pub version: String,
    /// Download URLs for each platform
    pub urls: HashMap<String, String>,
}

/// Cache structure for storing version information with timestamp
///
/// This structure is used to cache the list of available versions from the
/// OpenShift mirror to avoid repeated network requests. The cache expires
/// after 72 hours and is also updated when requested versions are not found.
#[derive(Serialize, Deserialize)]
pub struct VersionCache {
    /// List of available versions with platform URLs
    versions: Vec<VersionInfo>,
    /// Unix timestamp (seconds since epoch) when the cache was created
    timestamp: u64,
}

/// Legacy cache structure for backward compatibility with chrono timestamps
#[derive(Serialize, Deserialize)]
struct LegacyVersionCache {
    /// List of available versions (old format)
    versions: Vec<String>,
    /// Chrono timestamp (kept for deserializing old caches)
    timestamp: String,
}

/// Get current Unix timestamp (seconds since epoch)
fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl VersionCache {
    /// Create a new version cache with current timestamp
    ///
    /// # Arguments
    /// * `versions` - Vector of VersionInfo to cache
    #[must_use]
    pub fn new(versions: Vec<VersionInfo>) -> Self {
        Self {
            versions,
            timestamp: current_unix_timestamp(),
        }
    }

    /// Create a version cache with a specific timestamp.
    #[doc(hidden)]
    #[must_use]
    pub fn with_timestamp(versions: Vec<VersionInfo>, timestamp: u64) -> Self {
        Self {
            versions,
            timestamp,
        }
    }

    /// Get version strings only (for backward compatibility)
    ///
    /// # Returns
    /// Vector of version strings
    #[must_use]
    pub fn get_version_strings(&self) -> Vec<String> {
        self.versions.iter().map(|v| v.version.clone()).collect()
    }

    /// Get download URL for a specific version and platform
    ///
    /// # Arguments
    /// * `version` - Version to look up
    /// * `platform_name` - Platform name to look up
    ///
    /// # Returns
    /// `Some(url)` if found, `None` otherwise
    #[must_use]
    pub fn get_download_url(&self, version: &str, platform_name: &str) -> Option<String> {
        self.versions
            .iter()
            .find(|v| v.version == version)
            .and_then(|v| v.urls.get(platform_name))
            .cloned()
    }

    /// Check if a version exists in the cache
    ///
    /// # Arguments
    /// * `version` - Version to check
    ///
    /// # Returns
    /// `true` if the version exists in cache
    #[must_use]
    pub fn has_version(&self, version: &str) -> bool {
        self.versions.iter().any(|v| v.version == version)
    }

    /// Get the cache timestamp (Unix seconds)
    ///
    /// # Returns
    /// Unix timestamp when cache was created
    #[must_use]
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Check if the cache has exceeded its TTL (72 hours)
    #[must_use]
    pub fn is_expired(&self) -> bool {
        current_unix_timestamp().saturating_sub(self.timestamp) >= CACHE_TTL_SECS
    }
}

/// Get the cache directory path, creating it if it doesn't exist
///
/// Uses `$XDG_CACHE_HOME` if set, otherwise falls back to `$HOME/.cache`.
///
/// # Returns
/// Path to the cache directory
///
/// # Errors
/// Returns error if HOME environment variable is not set or directory creation fails
pub fn get_cache_dir() -> Result<PathBuf, Box<dyn Error>> {
    let cache_base = std::env::var("XDG_CACHE_HOME")
        .or_else(|_| std::env::var("HOME").map(|home| format!("{home}/.cache")))?;
    let cache_dir = PathBuf::from(cache_base).join("ovc");
    fs::create_dir_all(&cache_dir)?;
    Ok(cache_dir)
}

/// Get the full path to the version cache file
///
/// # Returns
/// Path to the versions.json cache file
///
/// # Errors
/// Returns error if the cache directory cannot be created
pub fn get_cache_file_path() -> Result<PathBuf, Box<dyn Error>> {
    Ok(get_cache_dir()?.join("versions.json"))
}

/// Load cached version data if it exists, ignoring TTL
///
/// Returns the cache even if expired. Use `load_cached_versions` for
/// TTL-aware loading.
///
/// # Returns
/// `Some(VersionCache)` if cache file exists and is parseable, `None` otherwise
///
/// # Errors
/// Returns error if the cache file exists but cannot be read
fn load_cached_versions_raw() -> Result<Option<VersionCache>, Box<dyn Error>> {
    let cache_file = get_cache_file_path()?;

    if !cache_file.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&cache_file)?;

    // Try to load new format first
    if let Ok(cache) = serde_json::from_str::<VersionCache>(&content) {
        return Ok(Some(cache));
    }

    // Try to load legacy format and migrate (uses current timestamp since old format varies)
    if let Ok(legacy_cache) = serde_json::from_str::<LegacyVersionCache>(&content) {
        // Migrate to new format with current timestamp
        let version_info = build_version_info(&legacy_cache.versions);
        let new_cache = VersionCache::new(version_info);

        // Save the migrated cache
        if save_cached_versions(&new_cache.versions).is_err() {
            // If saving fails, just continue with the migrated data
        }

        return Ok(Some(new_cache));
    }

    // If neither format works, remove the corrupted cache file
    let _ = fs::remove_file(&cache_file);
    Ok(None)
}

/// Load cached version data if it exists and has not expired
///
/// Returns `None` if the cache file doesn't exist, can't be parsed, or has
/// exceeded the 72-hour TTL.
///
/// # Returns
/// `Some(VersionCache)` if valid, non-expired cache exists, `None` otherwise
///
/// # Errors
/// Returns error if the cache file exists but cannot be read
pub fn load_cached_versions() -> Result<Option<VersionCache>, Box<dyn Error>> {
    match load_cached_versions_raw()? {
        Some(cache) if cache.is_expired() => Ok(None),
        other => Ok(other),
    }
}

/// Save version data to cache for future use
///
/// Serializes the version list with current timestamp and saves to cache file.
///
/// # Arguments
/// * `versions` - List of VersionInfo to cache
///
/// # Errors
/// Returns error if the cache file cannot be written
pub fn save_cached_versions(versions: &[VersionInfo]) -> Result<(), Box<dyn Error>> {
    let cache_file = get_cache_file_path()?;
    let cache = VersionCache::new(versions.to_vec());
    let content = serde_json::to_string_pretty(&cache)?;
    fs::write(&cache_file, content)?;
    Ok(())
}

/// Build version info with URLs for all supported platforms
///
/// # Arguments
/// * `version_strings` - List of version strings
///
/// # Returns
/// Vector of VersionInfo with URLs populated for all platforms
#[must_use]
pub fn build_version_info(version_strings: &[String]) -> Vec<VersionInfo> {
    let platforms = [Platform::LINUX_X86_64];

    version_strings
        .iter()
        .map(|version| {
            let mut urls = HashMap::new();
            for platform in &platforms {
                let url = platform.build_download_url(version);
                urls.insert(platform.name.to_string(), url);
            }
            VersionInfo {
                version: version.clone(),
                urls,
            }
        })
        .collect()
}

/// Fetch all versions from the API and cache them
///
/// # Arguments
/// * `verbose` - Whether to show progress information
///
/// # Returns
/// Vector of available version strings sorted by semantic version
///
/// # Errors
/// Returns error if the API request fails or the response cannot be parsed
pub fn fetch_and_cache_all_versions(verbose: bool) -> Result<Vec<String>, Box<dyn Error>> {
    let platform = Platform::detect();
    let url = platform.build_versions_url();
    let body = reqwest::blocking::get(&url)?.text()?;

    let mut versions = vec![];
    for line in body.lines() {
        if let Some(ver) = line.split('"').nth(1)
            && ver.ends_with('/')
            && ver.chars().next().is_some_and(|c| c.is_ascii_digit())
        {
            versions.push(ver.trim_end_matches('/').to_string());
        }
    }

    versions.sort_by(|a, b| compare_versions(a, b));

    // Save to cache for future use
    if let Err(e) = save_cached_versions(&build_version_info(&versions)) {
        // Don't fail the operation if caching fails, just log it in verbose mode
        if verbose {
            eprintln!("Warning: Failed to cache versions: {e}");
        }
    } else if verbose {
        eprintln!("Cached {} versions", versions.len());
    }

    Ok(versions)
}

/// Update cache when a specific version is not found
///
/// Fetches fresh data from the API and updates the cache, but only if the
/// requested version is not already in the cache.
///
/// # Arguments
/// * `missing_version` - The version that was not found in cache
/// * `verbose` - Whether to show progress information
///
/// # Returns
/// `true` if cache was updated, `false` if version was already in cache
///
/// # Errors
/// Returns error if the API request fails or cache cannot be updated
pub fn update_cache_for_missing_version(
    missing_version: &str,
    verbose: bool,
) -> Result<bool, Box<dyn Error>> {
    // Check if the version is already in cache (might have been added by another process)
    if let Some(cache) = load_cached_versions()?
        && cache.has_version(missing_version)
    {
        return Ok(false); // Version is now in cache, no update needed
    }

    if verbose {
        eprintln!("Version {missing_version} not found in cache, updating from API...");
    }

    // Fetch fresh data and update cache
    fetch_and_cache_all_versions(verbose)?;
    Ok(true)
}

/// Format cache age in human-readable format
///
/// Shows how long ago the cache was created.
///
/// # Arguments
/// * `timestamp` - Unix timestamp (seconds since epoch)
///
/// # Returns
/// Human-readable age (e.g. "2h ago" or "30m ago")
#[must_use]
pub fn format_cache_age(timestamp: u64) -> String {
    let now = current_unix_timestamp();
    let age_secs = now.saturating_sub(timestamp);

    let days = age_secs / 86400;
    let hours = (age_secs % 86400) / 3600;
    let minutes = (age_secs % 3600) / 60;
    let seconds = age_secs % 60;

    if days > 0 {
        format!("{days}d ago")
    } else if hours > 0 {
        format!("{hours}h ago")
    } else if minutes > 0 {
        format!("{minutes}m ago")
    } else {
        format!("{seconds}s ago")
    }
}

/// Check if a version exists using cached version info
///
/// This function first checks cached data, and optionally updates the cache
/// if the version is not found and update_if_missing is true.
///
/// # Arguments
/// * `version` - Version to check
/// * `platform` - Platform to check for
/// * `update_if_missing` - Whether to update cache if version not found
///
/// # Returns
/// `Some(true)` if found, `Some(false)` if not found after cache update, `None` if cache unavailable
///
/// # Errors
/// Returns error if cache cannot be loaded or updated
pub fn version_exists_in_cache(
    version: &str,
    platform: &Platform,
    update_if_missing: bool,
) -> Result<Option<bool>, Box<dyn Error>> {
    if let Some(cache) = load_cached_versions()? {
        let exists = cache.get_download_url(version, platform.name).is_some();
        if exists || !update_if_missing {
            return Ok(Some(exists));
        }

        // Version not found and we should update cache
        if update_cache_for_missing_version(version, false)? {
            // Check again after cache update
            if let Some(updated_cache) = load_cached_versions()? {
                let exists_after_update = updated_cache
                    .get_download_url(version, platform.name)
                    .is_some();
                return Ok(Some(exists_after_update));
            }
        }

        Ok(Some(false))
    } else {
        Ok(None)
    }
}

/// Get available versions without verbose output
///
/// # Errors
/// Returns error if versions cannot be fetched from cache or API
pub fn get_available_versions() -> Result<Vec<String>, Box<dyn Error>> {
    get_available_versions_with_verbose(false)
}

/// Get available versions from the OpenShift mirror with optional verbose output
///
/// Uses cached data if available and not expired (72-hour TTL). Fetches from the
/// mirror if no cache exists or the cache has expired.
///
/// # Arguments
/// * `verbose` - Whether to show cache status and fetch progress
///
/// # Returns
/// Vector of available version strings sorted by semantic version
///
/// # Errors
/// Returns error if versions cannot be fetched from cache or API
pub fn get_available_versions_with_verbose(verbose: bool) -> Result<Vec<String>, Box<dyn Error>> {
    // Use raw loader so we can print expiry/freshness messages
    if let Some(cache) = load_cached_versions_raw()? {
        if cache.is_expired() {
            if verbose {
                eprintln!(
                    "Cache expired (last updated: {}), refreshing...",
                    format_cache_age(cache.timestamp())
                );
            }
        } else {
            if verbose {
                eprintln!(
                    "Using cached versions (last updated: {})",
                    format_cache_age(cache.timestamp())
                );
            }
            return Ok(cache.get_version_strings());
        }
    } else if verbose {
        eprintln!("No cache found, fetching versions from API...");
    }

    // No valid cache, fetch from API
    fetch_and_cache_all_versions(verbose)
}
