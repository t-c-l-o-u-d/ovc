// GNU Affero General Public License v3.0 or later (see LICENSE or https://www.gnu.org/licenses/agpl.txt)
// Allow multiple crate versions for Windows-only dependencies (we only target Linux)
#![allow(clippy::multiple_crate_versions)]
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

use clap::Parser;
use flate2::read::GzDecoder;

use tar::Archive;

// Import from library
use ovc::cache::{
    get_available_versions, get_available_versions_with_verbose, load_cached_versions,
    update_cache_for_missing_version, version_exists_in_cache,
};
use ovc::{OC_BIN_DIR, Platform, compare_versions, find_matching_version, is_stable_version};

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
        cmd_list_available(&version_pattern, cli.verbose)
    } else if let Some(version_pattern) = cli.installed {
        cmd_list_installed(&version_pattern, cli.verbose)
    } else if let Some(version_pattern) = cli.prune {
        cmd_prune(&version_pattern, cli.verbose)
    } else {
        // Default action: download, but require a version
        match cli.target_version {
            Some(version) => cmd_download(Some(version), cli.verbose),
            None => Err("ovc: missing version\nTry 'ovc --help' for more information.".into()),
        }
    };

    // Handle errors by printing to stderr and exiting with non-zero status
    if let Err(e) = result {
        eprintln!("{e}");
        exit(1);
    }
}

// =============================================================================
// Command Implementation Functions
// =============================================================================

/// Check if a version matches the given version pattern
///
/// Performs proper version prefix matching by ensuring the pattern is followed
/// by a dot, dash, or is an exact match. This prevents "4.1" from matching "4.13"
/// while allowing "4.19.0" to match both "4.19.0.1" and "4.19.0-rc.1".
///
/// # Arguments
/// * `version` - Full version string to check (e.g. "4.13.58")
/// * `pattern` - Version pattern to match against (e.g. "4.1")
///
/// # Returns
/// `true` if the version matches the pattern properly
///
/// # Examples
/// * matches_version_pattern("4.1.0", "4.1") -> true
/// * matches_version_pattern("4.13.58", "4.1") -> false
/// * matches_version_pattern("4.19.3", "4.19") -> true
/// * matches_version_pattern("4.19.0-rc.1", "4.19.0") -> true
fn matches_version_pattern(version: &str, pattern: &str) -> bool {
    if version == pattern {
        return true;
    }

    // Check if version starts with pattern followed by a dot or dash
    version.starts_with(&format!("{pattern}.")) || version.starts_with(&format!("{pattern}-"))
}

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

    // Check for existing oc binary in PATH before proceeding
    if let Some(existing_oc_path) = check_existing_oc_in_path() {
        return Err(format!(
            "Error: Remove the existing oc binary found in ${{PATH}}: {}",
            existing_oc_path.display()
        )
        .into());
    }

    // Auto-detect platform
    let platform = Platform::detect();

    // Validate version format and resolve to full version
    let resolved_version = resolve_version(&input_version)?;

    if verbose && input_version != resolved_version {
        eprintln!("Resolved {input_version} to {resolved_version}");
    }

    let (path, downloaded, _download_url) =
        ensure_oc_binary_with_platform(&resolved_version, &platform, verbose)?;

    if verbose {
        if downloaded {
            eprintln!("Downloading: {resolved_version}");
            eprintln!("Downloaded to: {}", path.display());
        } else {
            eprintln!("Already installed: {resolved_version} ({})", path.display());
        }
    }

    // Always set as default
    set_default_oc_with_platform(&resolved_version, &platform)?;

    if verbose {
        eprintln!("Set as default: {resolved_version}");
    }

    // Only show warnings in verbose mode
    if verbose {
        check_path_warnings(verbose);
    }

    Ok(())
}

/// List installed versions matching a pattern
///
/// Shows all locally installed versions that match the given version pattern.
/// In verbose mode, also shows the full path to each binary.
///
/// # Arguments
/// * `version_pattern` - Version pattern to match (e.g. "4.19")
/// * `verbose` - Whether to show full paths
fn cmd_list_installed(version_pattern: &str, verbose: bool) -> Result<(), Box<dyn Error>> {
    // Validate minimum version format (must have at least major.minor)
    let parts: Vec<&str> = version_pattern.split('.').collect();
    if parts.len() < 2 {
        return Err("Version must include at least major and minor version (e.g. 4.19)".into());
    }

    let all_versions = list_installed_versions()?;

    // Filter versions that match the pattern
    let matching_versions: Vec<String> = all_versions
        .into_iter()
        .filter(|v| matches_version_pattern(v, version_pattern))
        .collect();

    if matching_versions.is_empty() {
        return Err(format!("No installed versions found matching {version_pattern}").into());
    }

    for version in matching_versions {
        if verbose {
            let path = get_bin_dir()?.join(format!("oc-{version}"));
            println!("{version} ({})", path.display());
        } else {
            println!("{version}");
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
/// * `version_pattern` - Version pattern to match (e.g. "4.19")
/// * `verbose` - Whether to show cache status and other details
fn cmd_list_available(version_pattern: &str, verbose: bool) -> Result<(), Box<dyn Error>> {
    // Validate minimum version format (must have at least major.minor)
    let parts: Vec<&str> = version_pattern.split('.').collect();
    if parts.len() < 2 {
        return Err("Version must include at least major and minor version (e.g. 4.19)".into());
    }

    let all_versions = get_available_versions_with_verbose(verbose)?;

    // Filter versions that match the pattern
    let matching_versions: Vec<String> = all_versions
        .into_iter()
        .filter(|v| matches_version_pattern(v, version_pattern))
        .collect();

    if matching_versions.is_empty() {
        return Err(format!("No versions found matching {version_pattern}").into());
    }

    for version in matching_versions {
        println!("{version}");
    }
    Ok(())
}

/// Remove installed versions matching a pattern
///
/// Finds all installed versions matching the pattern, shows what will be
/// removed, and then removes the binary files.
///
/// # Arguments
/// * `version_pattern` - Version pattern to match (e.g. "4.19")
/// * `verbose` - Whether to show detailed removal progress
fn cmd_prune(version_pattern: &str, verbose: bool) -> Result<(), Box<dyn Error>> {
    // Validate minimum version format (must have at least major.minor)
    let parts: Vec<&str> = version_pattern.split('.').collect();
    if parts.len() < 2 {
        return Err("Version must include at least major and minor version (e.g. 4.19)".into());
    }

    let installed_versions = list_installed_versions()?;

    // Filter versions that match the pattern
    let matching_versions: Vec<String> = installed_versions
        .into_iter()
        .filter(|v| matches_version_pattern(v, version_pattern))
        .collect();

    if matching_versions.is_empty() {
        return Err(format!("No installed versions found matching {version_pattern}").into());
    }

    // Remove the versions
    let bin_dir = get_bin_dir()?;
    for version in &matching_versions {
        let oc_path = bin_dir.join(format!("oc-{version}"));
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
///
/// # Arguments
/// * `verbose` - Whether to show debug information about PATH detection
fn check_path_warnings(verbose: bool) {
    let Ok(home) = std::env::var("HOME") else {
        eprintln!("Warning: HOME environment variable not set");
        return;
    };
    let local_bin = PathBuf::from(&home).join(".local/bin");
    let oc_symlink = local_bin.join("oc");

    // Check if oc binary exists in ~/.local/bin
    if !oc_symlink.exists() {
        eprintln!("Warning: oc binary not found in ~/.local/bin");
        eprintln!("Run 'ovc [VERSION]' to install a version and set it as default");
        return;
    }

    // Check if ~/.local/bin is in PATH
    if let Ok(path_var) = std::env::var("PATH") {
        let local_bin_canonical = match local_bin.canonicalize() {
            Ok(path) => path,
            Err(_) => local_bin.clone(), // fallback to original path if canonicalize fails
        };

        let is_in_path = path_var.split(':').any(|p| {
            if p.is_empty() {
                return false;
            }

            // Try to canonicalize the PATH entry for comparison
            let path_entry = Path::new(p);
            if let Ok(canonical_entry) = path_entry.canonicalize() {
                canonical_entry == local_bin_canonical
            } else {
                // Fallback to string comparison if canonicalization fails
                let path_buf = Path::new(p).to_path_buf();
                path_buf == local_bin
            }
        });

        if !is_in_path && verbose {
            eprintln!("Warning: ~/.local/bin is not in your ${{PATH}}");
        }
    } else {
        eprintln!("Warning: Could not read $PATH environment variable");
    }
}

// =============================================================================
// Version Resolution and Management Functions
// =============================================================================

/// Resolve a partial version to a full version
///
/// Takes a version like "4.19" and resolves it to the latest available
/// patch version like "4.19.3". If the input is already a full version,
/// returns it unchanged. Updates cache if no matching version is found.
///
/// # Arguments
/// * `input_version` - Version string to resolve (e.g. "4.19" or "4.19.0")
///
/// # Returns
/// Full version string (e.g. "4.19.3")
fn resolve_version(input_version: &str) -> Result<String, Box<dyn Error>> {
    // Validate minimum version format (must have at least major.minor)
    let parts: Vec<&str> = input_version.split('.').collect();
    if parts.len() < 2 {
        return Err("Version must include at least major and minor version (e.g. 4.19)".into());
    }

    // Check if it's already a full version (has patch number)
    if parts.len() >= 3 {
        return Ok(input_version.to_string());
    }

    // It's a partial version (major.minor), find the latest patch version
    let mut available_versions = get_available_versions()?;

    if let Some(latest_patch) = find_matching_version(input_version, &available_versions) {
        return Ok(latest_patch);
    }

    // No matching version found, try updating cache and search again
    if update_cache_for_missing_version(input_version, false)? {
        available_versions = get_available_versions()?;
        if let Some(latest_patch) = find_matching_version(input_version, &available_versions) {
            return Ok(latest_patch);
        }
    }

    Err(format!("No versions found matching {input_version}").into())
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
/// Uses cached URLs when available to avoid rebuilding URLs.
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
    let oc_path = bin_dir.join(format!("oc-{version}"));

    // Try to get URL from cache first, fallback to building it
    let download_url = if let Some(cache) = load_cached_versions()? {
        cache
            .get_download_url(version, platform.name)
            .unwrap_or_else(|| platform.build_download_url(version))
    } else {
        platform.build_download_url(version)
    };

    if oc_path.exists() {
        return Ok((oc_path, false, download_url)); // false = no download performed
    }

    // Check if version exists, preferring cache lookup with update if missing
    let version_exists = match version_exists_in_cache(version, platform, true)? {
        Some(exists) => exists,
        None => version_exists_on_mirror(version, platform)?,
    };

    if !version_exists {
        return Err(format!(
            "Version '{}' not found for platform {}",
            version, platform.name
        )
        .into());
    }

    if verbose {
        eprintln!("Downloading from: {download_url}");
    }
    download_and_extract_with_url(version, &oc_path, &download_url)?;
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
    let home = std::env::var("HOME")?;
    let bin_dir = PathBuf::from(&home).join(OC_BIN_DIR).join(platform.name);
    fs::create_dir_all(&bin_dir)?;
    Ok(bin_dir)
}

/// Download and extract the OpenShift client binary
///
/// Downloads the tar.gz archive from the specified URL, extracts the
/// oc binary, and sets appropriate file permissions.
///
/// # Arguments
/// * `_version` - Version being downloaded (for error messages, currently unused)
/// * `oc_path` - Target path for the extracted binary
/// * `download_url` - URL to download the binary from
fn download_and_extract_with_url(
    _version: &str,
    oc_path: &PathBuf,
    download_url: &str,
) -> Result<(), Box<dyn Error>> {
    let resp = reqwest::blocking::get(download_url)?;
    if !resp.status().is_success() {
        return Err(format!("Failed to download: {download_url}").into());
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

/// Set executable permissions on a file
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
/// First checks the cache for the version URL, then falls back to HTTP request.
///
/// # Arguments
/// * `version` - Version to check
/// * `platform` - Platform to check for
///
/// # Returns
/// `true` if the version exists on the mirror
fn version_exists_on_mirror(version: &str, platform: &Platform) -> Result<bool, Box<dyn Error>> {
    let client = reqwest::blocking::Client::new();

    // Try to get URL from cache first
    if let Some(cache) = load_cached_versions()?
        && let Some(url) = cache.get_download_url(version, platform.name)
    {
        let resp = client.head(&url).send()?;
        return Ok(resp.status().is_success());
    }

    // Fallback to building URL and checking
    let url = platform.build_download_url(version);
    let resp = client.head(&url).send()?;
    Ok(resp.status().is_success())
}

/// Set a specific version as the default OpenShift client
fn set_default_oc_with_platform(version: &str, platform: &Platform) -> Result<(), Box<dyn Error>> {
    let bin_dir = get_bin_dir_with_platform(platform)?;
    let oc_path = bin_dir.join(format!("oc-{version}"));

    // Ensure the binary exists (download if needed)
    if !oc_path.exists() {
        let (_, _, _) = ensure_oc_binary_with_platform(version, platform, false)?;
    }

    // Create ~/.local/bin directory and symlinks
    let home = std::env::var("HOME")?;
    let local_bin = PathBuf::from(&home).join(".local/bin");
    fs::create_dir_all(&local_bin)?;

    // Create symlinks (binary existence is already guaranteed above)
    let symlink_oc = local_bin.join("oc");
    let symlink_kubectl = local_bin.join("kubectl");

    // Remove existing symlinks (including broken ones)
    remove_if_exists(&symlink_oc)?;
    remove_if_exists(&symlink_kubectl)?;

    // Create new symlinks
    create_symlink(&oc_path, &symlink_oc)?;
    create_symlink(&oc_path, &symlink_kubectl)?;

    Ok(())
}

/// Remove a file or symlink if it exists
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

/// Create a symlink
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

/// Check for existing oc binary in PATH using the which crate
/// Ignores the oc binary in ~/.local/bin since that's managed by ovc itself.
/// # Returns: `Some(path)` if an oc binary is found in PATH (excluding ~/.local/bin), `None` otherwise
fn check_existing_oc_in_path() -> Option<PathBuf> {
    let path = which::which("oc").ok()?;

    // Get the ~/.local/bin directory to exclude it from conflicts
    let home = std::env::var("HOME").ok()?;
    let local_bin = PathBuf::from(&home).join(".local/bin");

    // If the found oc binary is in ~/.local/bin, ignore it (managed by ovc)
    if let Some(parent) = path.parent()
        && parent == local_bin
    {
        return None;
    }

    Some(path)
}
