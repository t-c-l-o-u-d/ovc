// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cache mirror version listings and download URLs, refreshed every 72 hours.

use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::{Platform, compare_versions, xdg};

/// Cache lifetime in seconds.
const CACHE_TTL_SECS: u64 = 72 * 60 * 60;

/// A version and its download URL for each platform.
#[derive(Serialize, Deserialize, Clone)]
pub struct VersionInfo {
    /// Version string, such as `4.19.0`.
    pub version: String,
    /// Download URL keyed by platform name.
    pub urls: HashMap<String, String>,
}

/// Cached version listing with its creation time.
#[derive(Serialize, Deserialize)]
pub struct VersionCache {
    /// Available versions with their platform URLs.
    versions: Vec<VersionInfo>,
    /// Unix timestamp when the cache was written.
    timestamp: u64,
}

/// Cache layout written by earlier releases, read to migrate it.
#[derive(Serialize, Deserialize)]
struct LegacyVersionCache {
    /// Available versions, without URLs.
    versions: Vec<String>,
    /// Timestamp stored as a formatted string.
    timestamp: String,
}

/// Get the current Unix timestamp in seconds.
fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl VersionCache {
    /// Create a cache stamped with the current time.
    #[must_use]
    pub fn new(versions: Vec<VersionInfo>) -> Self {
        Self {
            versions,
            timestamp: current_unix_timestamp(),
        }
    }

    /// Create a cache with a specific timestamp.
    #[doc(hidden)]
    #[must_use]
    pub fn with_timestamp(versions: Vec<VersionInfo>, timestamp: u64) -> Self {
        Self {
            versions,
            timestamp,
        }
    }

    /// Get the cached version strings.
    #[must_use]
    pub fn get_version_strings(&self) -> Vec<String> {
        self.versions.iter().map(|v| v.version.clone()).collect()
    }

    /// Get the download URL for a version and platform.
    #[must_use]
    pub fn get_download_url(&self, version: &str, platform_name: &str) -> Option<String> {
        self.versions
            .iter()
            .find(|v| v.version == version)
            .and_then(|v| v.urls.get(platform_name))
            .cloned()
    }

    /// Check whether a version is cached.
    #[must_use]
    pub fn has_version(&self, version: &str) -> bool {
        self.versions.iter().any(|v| v.version == version)
    }

    /// Get the cache timestamp in Unix seconds.
    #[must_use]
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Check whether the cache has outlived its lifetime.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        current_unix_timestamp().saturating_sub(self.timestamp) >= CACHE_TTL_SECS
    }
}

/// Get the cache directory, creating it when absent.
///
/// # Errors
/// Fails when HOME is unset or the directory cannot be created.
pub fn get_cache_dir() -> Result<PathBuf, Box<dyn Error>> {
    let cache_dir = xdg::base_dir("XDG_CACHE_HOME", ".cache")?.join("ovc");
    fs::create_dir_all(&cache_dir)?;
    Ok(cache_dir)
}

/// Get the path of the version cache file.
///
/// # Errors
/// Fails when the cache directory cannot be created.
pub fn get_cache_file_path() -> Result<PathBuf, Box<dyn Error>> {
    Ok(get_cache_dir()?.join("versions.json"))
}

/// Load the cache regardless of age, migrating an older layout.
///
/// An unreachable cache reads as an absent one.
fn load_cached_versions_raw(verbose: bool) -> Option<VersionCache> {
    let cache_file = match get_cache_file_path() {
        Ok(path) => path,
        Err(e) => {
            if verbose {
                eprintln!("Warning: Cannot open cache: {e}");
            }
            return None;
        }
    };

    if !cache_file.exists() {
        return None;
    }

    let content = match fs::read_to_string(&cache_file) {
        Ok(content) => content,
        Err(e) => {
            if verbose {
                eprintln!("Warning: Cannot read cache: {e}");
            }
            return None;
        }
    };

    if let Ok(cache) = serde_json::from_str::<VersionCache>(&content) {
        return Some(cache);
    }

    if let Ok(legacy) = serde_json::from_str::<LegacyVersionCache>(&content) {
        // Stamp the migration with the current time
        let migrated = VersionCache::new(build_version_info(&legacy.versions));
        if let Err(e) = save_cached_versions(&migrated.versions)
            && verbose
        {
            eprintln!("Warning: Cannot save migrated cache: {e}");
        }
        return Some(migrated);
    }

    if let Err(e) = fs::remove_file(&cache_file)
        && verbose
    {
        eprintln!("Warning: Cannot remove unreadable cache: {e}");
    }
    None
}

/// Load the cache, treating an expired one as absent.
#[must_use]
pub fn load_cached_versions(verbose: bool) -> Option<VersionCache> {
    match load_cached_versions_raw(verbose) {
        Some(cache) if cache.is_expired() => None,
        other => other,
    }
}

/// Write versions to the cache, stamped with the current time.
///
/// # Errors
/// Fails when the cache file cannot be written.
pub fn save_cached_versions(versions: &[VersionInfo]) -> Result<(), Box<dyn Error>> {
    let cache_file = get_cache_file_path()?;
    let cache = VersionCache::new(versions.to_vec());
    let content = serde_json::to_string_pretty(&cache)?;
    fs::write(&cache_file, content)?;
    Ok(())
}

/// Pair each version with a download URL per platform.
#[must_use]
pub fn build_version_info(version_strings: &[String]) -> Vec<VersionInfo> {
    let platforms = [Platform::LINUX_X86_64];

    version_strings
        .iter()
        .map(|version| {
            let mut urls = HashMap::new();
            for platform in &platforms {
                urls.insert(
                    platform.name.to_string(),
                    platform.build_download_url(version),
                );
            }
            VersionInfo {
                version: version.clone(),
                urls,
            }
        })
        .collect()
}

/// Fetch every version from the mirror and cache the result.
///
/// # Errors
/// Fails when the request fails or the listing cannot be parsed.
pub fn fetch_and_cache_versions(verbose: bool) -> Result<Vec<String>, Box<dyn Error>> {
    let url = Platform::detect().build_versions_url();
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

    // Fetched list stays usable if caching fails
    if let Err(e) = save_cached_versions(&build_version_info(&versions)) {
        if verbose {
            eprintln!("Warning: Cannot cache versions: {e}");
        }
    } else if verbose {
        eprintln!("Cached {} versions", versions.len());
    }

    Ok(versions)
}

/// Refresh the cache unless it already holds the version.
///
/// # Errors
/// Fails when the mirror request fails.
pub fn refresh_missing_version(
    missing_version: &str,
    verbose: bool,
) -> Result<bool, Box<dyn Error>> {
    if let Some(cache) = load_cached_versions(verbose)
        && cache.has_version(missing_version)
    {
        return Ok(false);
    }

    if verbose {
        eprintln!("Version {missing_version} missing from cache, refreshing");
    }

    fetch_and_cache_versions(verbose)?;
    Ok(true)
}

/// Format a cache age, such as `2h ago`.
#[must_use]
pub fn format_cache_age(timestamp: u64) -> String {
    let age_secs = current_unix_timestamp().saturating_sub(timestamp);

    let days = age_secs / 86400;
    let hours = (age_secs % 86400) / 3600;
    let minutes = (age_secs % 3600) / 60;
    let seconds = age_secs % 60;

    if days > 0 {
        format!("{days}d {hours}h ago")
    } else if hours > 0 {
        format!("{hours}h ago")
    } else if minutes > 0 {
        format!("{minutes}m ago")
    } else {
        format!("{seconds}s ago")
    }
}

/// Get available versions, preferring a fresh cache over the mirror.
///
/// # Errors
/// Fails when versions cannot be read from the cache or mirror.
pub fn available_versions(verbose: bool) -> Result<Vec<String>, Box<dyn Error>> {
    // Raw load so expiry can be reported
    if let Some(cache) = load_cached_versions_raw(verbose) {
        if cache.is_expired() {
            if verbose {
                eprintln!(
                    "Cache expired (last updated: {}), refreshing",
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
        eprintln!("No cache found, fetching versions from mirror");
    }

    fetch_and_cache_versions(verbose)
}
