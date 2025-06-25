//! OpenShift Client Version Control (ovc) - Main Application
//!
//! This is the main entry point for the ovc CLI tool, which provides functionality
//! for downloading, managing, and switching between different versions of the
//! OpenShift client (`oc`) binary.
//!
//! The application supports:
//! - Downloading specific versions of the OpenShift client
//! - Listing available versions from the mirror
//! - Managing installed versions locally
//! - Pruning old or unused versions
//! - Automatic platform detection
//! - Version caching for improved performance

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::exit;

use chrono::{DateTime, Duration, Utc};
use clap::Parser;
use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use tar::Archive;

// Import from library
use ovc::{OC_BIN_DIR, Platform, compare_versions, find_matching_version, is_stable_version};

/// Cache structure for storing version information with timestamp
///
/// This structure is used to cache the list of available versions from the
/// OpenShift mirror to avoid repeated network requests. The cache expires
/// after 1 hour to ensure reasonably fresh data.
#[derive(Serialize, Deserialize)]
struct VersionCache {
    /// List of available versions
    versions: Vec<String>,
    /// Timestamp when the cache was created
    timestamp: DateTime<Utc>,
}

impl VersionCache {
    /// Create a new version cache with current timestamp
    ///
    /// # Arguments
    /// * `versions` - Vector of version strings to cache
    fn new(versions: Vec<String>) -> Self {
        Self {
            versions,
            timestamp: Utc::now(),
        }
    }

    /// Check if the cache has expired (older than 1 hour)
    ///
    /// # Returns
    /// `true` if the cache is expired and should be refreshed
    fn is_expired(&self) -> bool {
        let now = Utc::now();
        let age = now.signed_duration_since(self.timestamp);
        age > Duration::hours(1)
    }
}

/// Command line interface structure
#[derive(Parser)]
#[command(name = "ovc", version, about = "openshift client version control")]
struct Cli {
    /// Version to download
    #[arg(value_name = "VERSION")]
    target_version: Option<String>,

    /// List available versions from the mirror
    #[arg(short = 'l', long = "list", value_name = "VERSION")]
    list: Option<String>,

    /// List installed versions
    #[arg(short = 'i', long = "installed", value_name = "VERSION")]
    installed: Option<String>,

    /// Remove installed versions
    #[arg(short = 'p', long = "prune", value_name = "VERSION")]
    prune: Option<String>,

    /// Make the operation more talkative
    #[arg(short, long)]
    verbose: bool,
}

/// Main application entry point
///
/// Parses command line arguments and dispatches to appropriate command handlers.
/// Ensures only one action is specified at a time and provides proper error handling.
fn main() {
    let cli = Cli::parse();

    // Count how many action flags are set to ensure mutual exclusivity
    let action_count = [
        cli.list.is_some(),
        cli.installed.is_some(),
        cli.prune.is_some(),
    ]
    .iter()
    .filter(|&&x| x)
    .count();

    // Dispatch to appropriate command handler
    let result = if action_count > 1 {
        Err("Only one action can be specified at a time".into())
    } else if let Some(version_pattern) = cli.list {
        cmd_list_available(version_pattern, cli.verbose)
    } else if let Some(version_pattern) = cli.installed {
        cmd_list_installed(version_pattern, cli.verbose)
    } else if let Some(version_pattern) = cli.prune {
        cmd_prune(version_pattern, cli.verbose)
    } else {
        // Default action: download, but require a version
        match cli.target_version {
            Some(version) => cmd_download(Some(version), cli.verbose),
            None => Err("ovc: missing version".into()),
        }
    };

    // Handle errors by printing to stderr and exiting with non-zero status
    if let Err(e) = result {
        eprintln!("{}", e);
        exit(1);
    }
}

// =============================================================================
// Cache Management Functions
// =============================================================================

/// Get the cache directory path, creating it if it doesn't exist
///
/// Creates the ~/.cache/ovc directory structure for storing cached data.
///
/// # Returns
/// Path to the cache directory
///
/// # Errors
/// Returns error if home directory cannot be found or directory creation fails
fn get_cache_dir() -> Result<PathBuf, Box<dyn Error>> {
    let cache_dir = dirs::home_dir()
        .ok_or("Could not find home directory")?
        .join(".cache")
        .join("ovc");
    fs::create_dir_all(&cache_dir)?;
    Ok(cache_dir)
}

/// Get the full path to the version cache file
///
/// # Returns
/// Path to the versions.json cache file
fn get_cache_file_path() -> Result<PathBuf, Box<dyn Error>> {
    Ok(get_cache_dir()?.join("versions.json"))
}

/// Load cached version data if it exists and is not expired
///
/// Attempts to load the version cache from disk. If the cache file doesn't exist
/// or has expired, returns None. Expired cache files are automatically removed.
///
/// # Returns
/// `Some(VersionCache)` if valid cache exists, `None` otherwise
fn load_cached_versions() -> Result<Option<VersionCache>, Box<dyn Error>> {
    let cache_file = get_cache_file_path()?;

    if !cache_file.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&cache_file)?;
    let cache: VersionCache = serde_json::from_str(&content)?;

    if cache.is_expired() {
        // Remove expired cache file
        let _ = fs::remove_file(&cache_file);
        return Ok(None);
    }

    Ok(Some(cache))
}

/// Save version data to cache for future use
///
/// Serializes the version list with current timestamp and saves to cache file.
///
/// # Arguments
/// * `versions` - List of version strings to cache
fn save_cached_versions(versions: &[String]) -> Result<(), Box<dyn Error>> {
    let cache_file = get_cache_file_path()?;
    let cache = VersionCache::new(versions.to_vec());
    let content = serde_json::to_string_pretty(&cache)?;
    fs::write(&cache_file, content)?;
    Ok(())
}

// =============================================================================
// Command Implementation Functions
// =============================================================================

/// Download and install a specific OpenShift client version
///
/// This is the main download command that:
/// 1. Resolves partial versions to full versions
/// 2. Downloads the binary if not already present
/// 3. Sets the downloaded version as the default
/// 4. Provides verbose output when requested
///
/// # Arguments
/// * `version` - Optional version to download (None for latest)
/// * `verbose` - Whether to provide detailed output
fn cmd_download(version: Option<String>, verbose: bool) -> Result<(), Box<dyn Error>> {
    let input_version = match version {
        Some(v) => v,
        None => get_latest_version()?,
    };

    // Auto-detect platform
    let platform = Platform::detect();

    // Validate version format and resolve to full version
    let resolved_version = resolve_version(&input_version)?;

    if verbose && input_version != resolved_version {
        eprintln!("Resolved {} to {}", input_version, resolved_version);
    }

    let (path, downloaded, _download_url) =
        ensure_oc_binary_with_platform(&resolved_version, &platform, verbose)?;

    if verbose {
        if downloaded {
            eprintln!("Downloading: {}", resolved_version);
            eprintln!("Downloaded to: {}", path.display());
        } else {
            eprintln!(
                "Already installed: {} ({})",
                resolved_version,
                path.display()
            );
        }
    }

    // Always set as default
    set_default_oc_with_platform(&resolved_version, &platform)?;

    if verbose {
        eprintln!("Set as default: {}", resolved_version);
    }

    // Only show warnings in verbose mode
    if verbose {
        check_path_warnings()?;
    }

    Ok(())
}

/// List installed versions matching a pattern
///
/// Shows all locally installed versions that match the given version pattern.
/// In verbose mode, also shows the full path to each binary.
///
/// # Arguments
/// * `version_pattern` - Version pattern to match (e.g., "4.19")
/// * `verbose` - Whether to show full paths
fn cmd_list_installed(version_pattern: String, verbose: bool) -> Result<(), Box<dyn Error>> {
    // Validate minimum version format (must have at least major.minor)
    let parts: Vec<&str> = version_pattern.split('.').collect();
    if parts.len() < 2 {
        return Err("Version must include at least major and minor version (e.g., 4.19)".into());
    }

    let all_versions = list_installed_versions()?;

    // Filter versions that match the pattern
    let matching_versions: Vec<String> = all_versions
        .into_iter()
        .filter(|v| v.starts_with(&version_pattern))
        .collect();

    if matching_versions.is_empty() {
        return Err(format!("No installed versions found matching {}", version_pattern).into());
    }

    for version in matching_versions {
        if verbose {
            let path = get_bin_dir()?.join(format!("oc-{}", version));
            println!("{} ({})", version, path.display());
        } else {
            println!("{}", version);
        }
    }
    Ok(())
}

/// List available versions from the mirror matching a pattern
///
/// Queries the OpenShift mirror for available versions and shows those
/// matching the given pattern. Uses caching to improve performance.
///
/// # Arguments
/// * `version_pattern` - Version pattern to match (e.g., "4.19")
/// * `verbose` - Whether to show cache status and other details
fn cmd_list_available(version_pattern: String, verbose: bool) -> Result<(), Box<dyn Error>> {
    // Validate minimum version format (must have at least major.minor)
    let parts: Vec<&str> = version_pattern.split('.').collect();
    if parts.len() < 2 {
        return Err("Version must include at least major and minor version (e.g., 4.19)".into());
    }

    let all_versions = get_available_versions_with_verbose(verbose)?;

    // Filter versions that match the pattern
    let matching_versions: Vec<String> = all_versions
        .into_iter()
        .filter(|v| v.starts_with(&version_pattern))
        .collect();

    if matching_versions.is_empty() {
        return Err(format!("No versions found matching {}", version_pattern).into());
    }

    for version in matching_versions {
        println!("{}", version);
    }
    Ok(())
}

/// Remove installed versions matching a pattern
///
/// Finds all installed versions matching the pattern, shows what will be
/// removed, and then removes the binary files.
///
/// # Arguments
/// * `version_pattern` - Version pattern to match (e.g., "4.19")
/// * `verbose` - Whether to show detailed removal progress
fn cmd_prune(version_pattern: String, verbose: bool) -> Result<(), Box<dyn Error>> {
    // Validate minimum version format (must have at least major.minor)
    let parts: Vec<&str> = version_pattern.split('.').collect();
    if parts.len() < 2 {
        return Err("Version must include at least major and minor version (e.g., 4.19)".into());
    }

    let installed_versions = list_installed_versions()?;

    // Filter versions that match the pattern
    let matching_versions: Vec<String> = installed_versions
        .into_iter()
        .filter(|v| v.starts_with(&version_pattern))
        .collect();

    if matching_versions.is_empty() {
        return Err(format!("No installed versions found matching {}", version_pattern).into());
    }

    // Show what will be removed
    println!("Will remove the following:");
    for version in &matching_versions {
        println!("{}", version);
    }

    // Remove the versions
    let bin_dir = get_bin_dir()?;
    for version in &matching_versions {
        let oc_path = bin_dir.join(format!("oc-{}", version));
        if oc_path.exists() {
            if verbose {
                eprintln!("Removing: {}", oc_path.display());
            }
            std::fs::remove_file(&oc_path)?;
        }
    }

    if verbose {
        eprintln!("Removed {} version(s)", matching_versions.len());
    }

    Ok(())
}

/// Check for common PATH and installation issues
///
/// Warns the user if:
/// - The oc binary is not found in ~/.local/bin
/// - ~/.local/bin is not in the user's PATH
///
/// This helps users understand why the oc command might not be available.
fn check_path_warnings() -> Result<(), Box<dyn Error>> {
    let local_bin = dirs::home_dir()
        .ok_or("Could not find home directory")?
        .join(".local/bin");
    let oc_symlink = local_bin.join("oc");

    // Check if oc binary exists in ~/.local/bin
    if !oc_symlink.exists() {
        eprintln!("Warning: oc binary not found in ~/.local/bin");
        eprintln!("Run 'ovc download' to install a version and set it as default");
        return Ok(());
    }

    // Check if ~/.local/bin is in PATH
    if let Ok(path_var) = std::env::var("PATH") {
        let local_bin_str = local_bin.to_string_lossy();
        let is_in_path = path_var.split(':').any(|p| p == local_bin_str);

        if !is_in_path {
            eprintln!("Warning: ~/.local/bin is not in your PATH");
        }
    }

    Ok(())
}

// =============================================================================
// Version Resolution and Management Functions
// =============================================================================

/// Resolve a partial version to a full version
///
/// Takes a version like "4.19" and resolves it to the latest available
/// patch version like "4.19.3". If the input is already a full version,
/// returns it unchanged.
///
/// # Arguments
/// * `input_version` - Version string to resolve (e.g., "4.19" or "4.19.0")
///
/// # Returns
/// Full version string (e.g., "4.19.3")
fn resolve_version(input_version: &str) -> Result<String, Box<dyn Error>> {
    // Validate minimum version format (must have at least major.minor)
    let parts: Vec<&str> = input_version.split('.').collect();
    if parts.len() < 2 {
        return Err("Version must include at least major and minor version (e.g., 4.19)".into());
    }

    // Check if it's already a full version (has patch number)
    if parts.len() >= 3 {
        return Ok(input_version.to_string());
    }

    // It's a partial version (major.minor), find the latest patch version
    let available_versions = get_available_versions()?;

    if let Some(latest_patch) = find_matching_version(input_version, &available_versions) {
        Ok(latest_patch)
    } else {
        Err(format!("No versions found matching {}", input_version).into())
    }
}

/// Get the latest stable version available
///
/// Fetches all available versions and returns the latest stable (non-prerelease)
/// version. Filters out alpha, beta, rc, and other prerelease versions.
///
/// # Returns
/// Latest stable version string
fn get_latest_version() -> Result<String, Box<dyn Error>> {
    let versions = get_available_versions()?;

    // Filter out pre-release versions (rc, alpha, beta, nightly, etc.)
    let stable_versions: Vec<String> = versions
        .into_iter()
        .filter(|v| is_stable_version(v))
        .collect();

    stable_versions
        .last()
        .cloned()
        .ok_or_else(|| "No stable versions found".into())
}

// =============================================================================
// Binary Management Functions
// =============================================================================

/// Ensure the OpenShift client binary is available for the specified version and platform
///
/// Checks if the binary already exists locally. If not, downloads and extracts it.
/// Returns information about the binary path and whether a download occurred.
///
/// # Arguments
/// * `version` - Version to ensure is available
/// * `platform` - Target platform for the binary
/// * `verbose` - Whether to show download progress
///
/// # Returns
/// Tuple of (binary_path, was_downloaded, download_url)
fn ensure_oc_binary_with_platform(
    version: &str,
    platform: &Platform,
    verbose: bool,
) -> Result<(PathBuf, bool, String), Box<dyn Error>> {
    let bin_dir = get_bin_dir_with_platform(platform)?;
    let oc_path = bin_dir.join(format!("oc-{}", version));
    let download_url = platform.build_download_url(version);

    if oc_path.exists() {
        return Ok((oc_path, false, download_url)); // false = no download performed
    }

    if !version_exists_on_mirror(version, platform)? {
        return Err(format!(
            "Version '{}' not found for platform {}",
            version, platform.name
        )
        .into());
    }

    if verbose {
        eprintln!("Downloading from: {}", download_url);
    }
    download_and_extract(version, &oc_path, platform)?;
    Ok((oc_path, true, download_url)) // true = download performed
}

/// Get the binary directory for the current platform
fn get_bin_dir() -> Result<PathBuf, Box<dyn Error>> {
    let platform = Platform::detect();
    get_bin_dir_with_platform(&platform)
}

/// Get the binary directory for a specific platform
///
/// Creates the directory structure if it doesn't exist.
///
/// # Arguments
/// * `platform` - Platform to get directory for
///
/// # Returns
/// Path to the platform-specific binary directory
fn get_bin_dir_with_platform(platform: &Platform) -> Result<PathBuf, Box<dyn Error>> {
    let bin_dir = dirs::home_dir()
        .ok_or("Could not find home directory")?
        .join(OC_BIN_DIR)
        .join(platform.name);
    fs::create_dir_all(&bin_dir)?;
    Ok(bin_dir)
}

/// Download and extract the OpenShift client binary
///
/// Downloads the tar.gz archive from the OpenShift mirror, extracts the
/// oc binary, and sets appropriate file permissions.
///
/// # Arguments
/// * `version` - Version being downloaded
/// * `oc_path` - Target path for the extracted binary
/// * `platform` - Platform information for building download URL
fn download_and_extract(
    version: &str,
    oc_path: &PathBuf,
    platform: &Platform,
) -> Result<(), Box<dyn Error>> {
    let url = platform.build_download_url(version);
    let resp = Client::new().get(&url).send()?;

    if !resp.status().is_success() {
        return Err(format!("Failed to download: {}", url).into());
    }

    let tar_gz = GzDecoder::new(resp);
    let mut archive = Archive::new(tar_gz);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        if path.ends_with("oc") {
            let mut out = fs::File::create(oc_path)?;
            io::copy(&mut entry, &mut out)?;
            set_executable(oc_path)?;
            return Ok(());
        }
    }

    Err("oc binary not found in archive".into())
}

/// Set executable permissions on a file (Unix only)
#[cfg(unix)]
fn set_executable(path: &PathBuf) -> Result<(), Box<dyn Error>> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

/// List all locally installed OpenShift client versions
///
/// Scans the binary directory for files matching the "oc-{version}" pattern
/// and returns a sorted list of versions.
///
/// # Returns
/// Vector of version strings sorted by semantic version
fn list_installed_versions() -> Result<Vec<String>, Box<dyn Error>> {
    let bin_dir = get_bin_dir()?;
    let mut versions = vec![];

    if bin_dir.exists() {
        for entry in fs::read_dir(bin_dir)? {
            let entry = entry?;
            let fname = entry.file_name().into_string().unwrap();
            if let Some(version) = fname.strip_prefix("oc-") {
                versions.push(version.to_string());
            }
        }
    }

    versions.sort_by(|a, b| compare_versions(a, b));

    Ok(versions)
}

/// Check if a version exists on the OpenShift mirror
///
/// Performs a HEAD request to check if the download URL returns a successful response.
///
/// # Arguments
/// * `version` - Version to check
/// * `platform` - Platform to check for
///
/// # Returns
/// `true` if the version exists on the mirror
fn version_exists_on_mirror(version: &str, platform: &Platform) -> Result<bool, Box<dyn Error>> {
    let url = platform.build_download_url(version);
    let resp = Client::new().head(&url).send()?;
    Ok(resp.status().is_success())
}

// =============================================================================
// Version Fetching and Caching Functions
// =============================================================================

/// Get available versions without verbose output
fn get_available_versions() -> Result<Vec<String>, Box<dyn Error>> {
    get_available_versions_with_verbose(false)
}

/// Get available versions from the OpenShift mirror with optional verbose output
///
/// First attempts to load from cache. If cache is missing or expired,
/// fetches fresh data from the mirror and updates the cache.
///
/// # Arguments
/// * `verbose` - Whether to show cache status and fetch progress
///
/// # Returns
/// Vector of available version strings sorted by semantic version
fn get_available_versions_with_verbose(verbose: bool) -> Result<Vec<String>, Box<dyn Error>> {
    // Try to load from cache first
    if let Some(cache) = load_cached_versions()? {
        if verbose {
            eprintln!(
                "Using cached versions (expires in {})",
                format_cache_expiry(&cache.timestamp)
            );
        }
        return Ok(cache.versions);
    }

    if verbose {
        eprintln!("Fetching versions from API...");
    }

    // Cache miss or expired, fetch from API
    let platform = Platform::detect();
    let url = platform.build_versions_url();
    let resp = Client::new().get(&url).send()?;
    let body = resp.text()?;

    let mut versions = vec![];
    for line in body.lines() {
        if let Some(ver) = line.split('"').nth(1) {
            if ver.ends_with('/') && ver.chars().next().unwrap().is_ascii_digit() {
                versions.push(ver.trim_end_matches('/').to_string());
            }
        }
    }

    versions.sort_by(|a, b| compare_versions(a, b));

    // Save to cache for future use
    if let Err(e) = save_cached_versions(&versions) {
        // Don't fail the operation if caching fails, just log it in verbose mode
        if verbose {
            eprintln!("Warning: Failed to cache versions: {}", e);
        }
    } else if verbose {
        eprintln!("Cached {} versions for 1 hour", versions.len());
    }

    Ok(versions)
}

/// Format cache expiry time in human-readable format
///
/// Shows remaining time until cache expires in minutes or seconds.
///
/// # Arguments
/// * `timestamp` - Cache creation timestamp
///
/// # Returns
/// Human-readable time remaining (e.g., "45m" or "30s")
fn format_cache_expiry(timestamp: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let expires_at = *timestamp + Duration::hours(1);
    let remaining = expires_at.signed_duration_since(now);

    if remaining.num_minutes() > 0 {
        format!("{}m", remaining.num_minutes())
    } else {
        format!("{}s", remaining.num_seconds().max(0))
    }
}

// =============================================================================
// Symlink Management Functions
// =============================================================================

/// Set a specific version as the default OpenShift client
///
/// Creates symlinks in ~/.local/bin pointing to the specified version.
/// Creates both 'oc' and 'kubectl' symlinks for compatibility.
///
/// # Arguments
/// * `version` - Version to set as default
/// * `platform` - Platform information
fn set_default_oc_with_platform(version: &str, platform: &Platform) -> Result<(), Box<dyn Error>> {
    let bin_dir = get_bin_dir_with_platform(platform)?;
    let oc_path = bin_dir.join(format!("oc-{}", version));

    if !oc_path.exists() {
        let (_, _, _) = ensure_oc_binary_with_platform(version, platform, false)?;
    }

    let local_bin = dirs::home_dir()
        .ok_or("Could not find home directory")?
        .join(".local/bin");
    fs::create_dir_all(&local_bin)?;

    create_symlinks(&oc_path, &local_bin)?;
    Ok(())
}

/// Create symlinks for both oc and kubectl commands
///
/// Removes any existing symlinks and creates fresh ones pointing to the
/// specified binary. This ensures both 'oc' and 'kubectl' commands work.
///
/// # Arguments
/// * `oc_path` - Path to the target oc binary
/// * `local_bin` - Directory to create symlinks in
fn create_symlinks(oc_path: &Path, local_bin: &Path) -> Result<(), Box<dyn Error>> {
    let symlink_oc = local_bin.join("oc");
    let symlink_kubectl = local_bin.join("kubectl");

    // Ensure the target binary exists before creating symlinks
    if !oc_path.exists() {
        return Err(format!("Target binary does not exist: {}", oc_path.display()).into());
    }

    // Remove existing symlinks (including broken ones)
    remove_if_exists(&symlink_oc)?;
    remove_if_exists(&symlink_kubectl)?;

    // Create new symlinks
    create_symlink(oc_path, &symlink_oc)?;
    create_symlink(oc_path, &symlink_kubectl)?;

    Ok(())
}

/// Remove a file or symlink if it exists
///
/// Safely removes files, directories, or symlinks (including broken ones).
/// Does nothing if the path doesn't exist.
///
/// # Arguments
/// * `path` - Path to remove
fn remove_if_exists(path: &Path) -> Result<(), Box<dyn Error>> {
    // Check if the path exists as a file, directory, or symlink (including broken symlinks)
    if path.exists() || path.is_symlink() {
        fs::remove_file(path).map_err(|e| {
            format!(
                "Failed to remove existing file/symlink at {}: {}",
                path.display(),
                e
            )
        })?;
    }
    Ok(())
}

/// Create a symlink (Unix implementation)
#[cfg(unix)]
fn create_symlink(target: &Path, link: &Path) -> Result<(), Box<dyn Error>> {
    std::os::unix::fs::symlink(target, link).map_err(|e| {
        format!(
            "Failed to create symlink {} -> {}: {}",
            link.display(),
            target.display(),
            e
        )
    })?;
    Ok(())
}
