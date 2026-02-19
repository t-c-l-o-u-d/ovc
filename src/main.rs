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
use std::process::{Command, exit};

use clap::Parser;
use flate2::read::GzDecoder;

use tar::Archive;

// Import from library
use ovc::cache::{
    get_available_versions, get_available_versions_with_verbose, load_cached_versions,
    update_cache_for_missing_version, version_exists_in_cache,
};
use ovc::{
    OC_BIN_DIR, Platform, compare_versions, find_matching_version, is_stable_version,
    matches_version_pattern,
};

/// Standalone actions that don't require a version argument
#[derive(Clone, Copy, PartialEq, Eq)]
enum StandaloneAction {
    MatchServer,
    Update,
}

/// CLI argument parser - bools required for clap flag parsing
#[derive(Parser)]
#[command(
    name = "ovc",
    version,
    about = "OpenShift Client Version Control",
    disable_version_flag = true
)]
#[command(arg(clap::Arg::new("version").long("version").action(clap::ArgAction::Version).help("Print version")))]
#[allow(clippy::struct_excessive_bools)]
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

    /// Download the version matching the currently connected cluster
    #[arg(short = 'm', long = "match-server", conflicts_with_all = ["update", "list", "installed", "prune"])]
    match_server: bool,

    /// Update ovc to the latest version from GitHub releases
    #[arg(short = 'u', long = "update", conflicts_with_all = ["match_server", "list", "installed", "prune"])]
    update: bool,

    /// Allow insecure TLS connections (skip certificate verification)
    #[arg(short = 'k', long = "insecure")]
    insecure: bool,

    /// Make the operation more talkative
    #[arg(short, long)]
    verbose: bool,

    /// Generate shell completion script (only bash is supported currently)
    #[arg(long = "completion", value_name = "SHELL", value_parser = parse_completion_shell)]
    completion: Option<String>,
}

impl Cli {
    fn standalone_action(&self) -> Option<StandaloneAction> {
        match (self.match_server, self.update) {
            (true, _) => Some(StandaloneAction::MatchServer),
            (_, true) => Some(StandaloneAction::Update),
            _ => None,
        }
    }
}

fn parse_completion_shell(s: &str) -> Result<String, String> {
    match s.to_lowercase().as_str() {
        "bash" => Ok(s.to_lowercase()),
        _ => Err(format!("unsupported shell: {s} (only 'bash' is supported)")),
    }
}

/// Main application entry point
///
/// Parses command line arguments and dispatches to appropriate command handlers.
/// Ensures only one action is specified at a time and provides proper error handling.
fn main() {
    let cli = Cli::parse();

    // Handle completion generation first (exits immediately)
    if cli.completion.is_some() {
        print_bash_completion();
        return;
    }

    let standalone = cli.standalone_action();
    let verbose = cli.verbose;
    let insecure = cli.insecure;

    // Dispatch to appropriate command handler
    // Note: conflicts_with_all ensures mutual exclusivity at parse time
    let result = if let Some(version_pattern) = cli.list {
        cmd_list_available(&version_pattern, verbose)
    } else if let Some(version_pattern) = cli.installed {
        cmd_list_installed(&version_pattern, verbose)
    } else if let Some(version_pattern) = cli.prune {
        cmd_prune(&version_pattern, verbose)
    } else if let Some(action) = standalone {
        match action {
            StandaloneAction::MatchServer => cmd_match_server(verbose, insecure),
            StandaloneAction::Update => cmd_update(verbose),
        }
    } else {
        // Default action: download, but require a version
        match cli.target_version {
            Some(version) => cmd_download(Some(version), verbose),
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

/// Download and install the oc binary directly from the connected cluster
///
/// Gets the console URL and downloads the oc binary from the cluster's downloads endpoint.
/// This ensures the client version exactly matches the connected cluster.
fn cmd_match_server(verbose: bool, insecure: bool) -> Result<(), Box<dyn Error>> {
    // Check for existing oc binary in PATH before proceeding
    if let Some(existing_oc_path) = check_existing_oc_in_path() {
        return Err(format!(
            "Error: Remove the existing oc binary found in ${{PATH}}: {}",
            existing_oc_path.display()
        )
        .into());
    }

    let download_url = get_cluster_download_url(verbose)?;

    if verbose {
        eprintln!("Downloading from cluster: {download_url}");
    }

    // Download to a temporary location first
    let platform = Platform::detect();
    let bin_dir = get_bin_dir_with_platform(&platform)?;
    let temp_path = bin_dir.join("oc-cluster-temp");

    download_oc_from_cluster(&download_url, &temp_path, insecure, verbose)?;

    // Get the version from the downloaded binary
    let version = get_binary_version(&temp_path)?;

    if verbose {
        eprintln!("Detected version: {version}");
    }

    // Move to final location with version in name
    let final_path = bin_dir.join(format!("oc-{version}"));
    fs::rename(&temp_path, &final_path)?;

    // Set as default
    set_default_oc_with_platform(&version, &platform)?;

    if verbose {
        eprintln!("Installed and set as default: {version}");
        check_path_warnings(verbose);
    }

    Ok(())
}

/// GitHub repository owner
const GITHUB_OWNER: &str = "t-c-l-o-u-d";
/// GitHub repository name
const GITHUB_REPO: &str = "ovc";

/// Update ovc to the latest version from GitHub releases
///
/// Checks the latest release on GitHub and updates if a newer version is available.
/// If already on the latest version, prints a message and shows the current version.
fn cmd_update(verbose: bool) -> Result<(), Box<dyn Error>> {
    let current_version = env!("CARGO_PKG_VERSION");

    if verbose {
        eprintln!("Current version: {current_version}");
        eprintln!("Checking for updates...");
    }

    let (latest_version, download_url) = get_latest_github_release(verbose)?;

    if verbose {
        eprintln!("Latest version: {latest_version}");
    }

    // Compare versions
    if compare_versions(&latest_version, current_version) != std::cmp::Ordering::Greater {
        println!("ovc is already up to date (version {current_version})");
        return Ok(());
    }

    if verbose {
        eprintln!("Downloading update from: {download_url}");
    }

    // Get the path to the current executable
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Failed to get current executable path: {e}"))?;

    // Download to a temporary file
    let temp_path = current_exe.with_extension("update");
    download_update(&download_url, &temp_path)?;

    // Replace current binary with the new one
    replace_binary(&temp_path, &current_exe)?;

    println!("Updated ovc from {current_version} to {latest_version}");
    Ok(())
}

/// Fetch the latest release information from GitHub
fn get_latest_github_release(verbose: bool) -> Result<(String, String), Box<dyn Error>> {
    let api_url =
        format!("https://api.github.com/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases/latest");

    if verbose {
        eprintln!("Fetching release info from: {api_url}");
    }

    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("ovc/{}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let resp = client.get(&api_url).send()?;

    if !resp.status().is_success() {
        return Err(format!(
            "Failed to fetch release info: {} ({})",
            api_url,
            resp.status()
        )
        .into());
    }

    let release: serde_json::Value = serde_json::from_str(&resp.text()?)?;

    // Get the tag name (version)
    let tag_name = release["tag_name"]
        .as_str()
        .ok_or("No tag_name in release")?;

    // Strip 'v' prefix if present
    let version = tag_name.strip_prefix('v').unwrap_or(tag_name);

    // Find the linux-x86_64 asset
    let assets = release["assets"].as_array().ok_or("No assets in release")?;

    let download_url = assets
        .iter()
        .find_map(|asset| {
            let name = asset["name"].as_str()?;
            if name.contains("linux") && (name.contains("x86_64") || name.contains("amd64")) {
                asset["browser_download_url"].as_str().map(String::from)
            } else {
                None
            }
        })
        .ok_or("No linux-x86_64 binary found in release assets")?;

    Ok((version.to_string(), download_url))
}

/// Download the update binary
fn download_update(url: &str, dest: &Path) -> Result<(), Box<dyn Error>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("ovc/{}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let resp = client.get(url).send()?;

    if !resp.status().is_success() {
        return Err(format!("Failed to download update: {} ({})", url, resp.status()).into());
    }

    let bytes = resp.bytes()?;
    fs::write(dest, &bytes)?;
    set_executable(&dest.to_path_buf())?;

    Ok(())
}

/// Replace the current binary with the new one
fn replace_binary(new_binary: &Path, current_binary: &Path) -> Result<(), Box<dyn Error>> {
    // On Unix, we can replace a running binary by:
    // 1. Rename current to .old
    // 2. Move new to current location
    // 3. Remove .old
    let old_path = current_binary.with_extension("old");

    // Remove old backup if it exists
    let _ = fs::remove_file(&old_path);

    // Rename current to .old
    fs::rename(current_binary, &old_path)
        .map_err(|e| format!("Failed to backup current binary: {e}"))?;

    // Move new binary to current location
    if let Err(e) = fs::rename(new_binary, current_binary) {
        // Try to restore the old binary
        let _ = fs::rename(&old_path, current_binary);
        return Err(format!("Failed to install new binary: {e}").into());
    }

    // Remove old binary
    let _ = fs::remove_file(&old_path);

    Ok(())
}

/// Get the download URL for oc binary from the connected cluster
///
/// Uses `oc whoami --show-console` to get the console URL, then transforms it
/// to the downloads endpoint URL.
fn get_cluster_download_url(verbose: bool) -> Result<String, Box<dyn Error>> {
    let output = Command::new("oc")
        .args(["whoami", "--show-console"])
        .output()
        .map_err(|e| format!("Failed to run 'oc': {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Not connected to a cluster. Run 'oc login' first.\n{stderr}").into());
    }

    let console_url = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if verbose {
        eprintln!("Console URL: {console_url}");
    }

    // Transform console URL to downloads URL
    // Example: https://console-openshift-console.apps-crc.testing
    //       -> https://downloads-openshift-console.apps-crc.testing/amd64/linux/oc.tar
    let download_url =
        console_url.replace("console-openshift-console", "downloads-openshift-console");
    let download_url = format!("{download_url}/amd64/linux/oc.tar");

    Ok(download_url)
}

/// Download the oc binary from the cluster's downloads endpoint
fn download_oc_from_cluster(
    url: &str,
    dest: &Path,
    insecure: bool,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    let client = reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(insecure)
        .build()?;

    let resp = match client.get(url).send() {
        Ok(r) => r,
        Err(e) => {
            if verbose && e.is_connect() {
                let err_str = e.to_string().to_lowercase();
                if err_str.contains("certificate")
                    || err_str.contains("ssl")
                    || err_str.contains("tls")
                {
                    eprintln!(
                        "Connection failed due to untrusted TLS certificate.\n\
                         The cluster may be using a self-signed certificate.\n\
                         Use --insecure (-k) to skip certificate verification."
                    );
                }
            }
            return Err(e.into());
        }
    };

    if !resp.status().is_success() {
        return Err(format!(
            "Failed to download from cluster: {} ({})",
            url,
            resp.status()
        )
        .into());
    }

    let mut archive = Archive::new(resp);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        if path.ends_with("oc") {
            let mut out = fs::File::create(dest)?;
            io::copy(&mut entry, &mut out)?;
            set_executable(&dest.to_path_buf())?;
            return Ok(());
        }
    }

    Err("oc binary not found in archive".into())
}

/// Get the version string from an oc binary
fn get_binary_version(path: &Path) -> Result<String, Box<dyn Error>> {
    let output = Command::new(path)
        .arg("version")
        .arg("--client")
        .output()
        .map_err(|e| format!("Failed to run downloaded oc binary: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse version from output like "Client Version: 4.16.55"
    for line in stdout.lines() {
        if let Some(version) = line.strip_prefix("Client Version: ") {
            return Ok(version.trim().to_string());
        }
    }

    Err("Could not determine version from downloaded binary".into())
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

/// Check for existing oc binary in PATH
/// Ignores the oc binary in ~/.local/bin since that's managed by ovc itself.
/// # Returns: `Some(path)` if an oc binary is found in PATH (excluding ~/.local/bin), `None` otherwise
fn check_existing_oc_in_path() -> Option<PathBuf> {
    let path_var = std::env::var("PATH").ok()?;
    let home = std::env::var("HOME").ok()?;
    let local_bin = PathBuf::from(&home).join(".local/bin");

    for dir in path_var.split(':') {
        if dir.is_empty() {
            continue;
        }

        let candidate = Path::new(dir).join("oc");

        // Skip if this is in ~/.local/bin (managed by ovc)
        if Path::new(dir) == local_bin {
            continue;
        }

        // Check if the file exists and is executable
        if candidate.is_file() {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = candidate.metadata()
                && metadata.permissions().mode() & 0o111 != 0
            {
                return Some(candidate);
            }
        }
    }

    None
}

/// Print bash completion script
fn print_bash_completion() {
    print!(
        r#"# bash completion for ovc

_ovc_completions() {{
    local cur prev
    COMPREPLY=()
    cur="${{COMP_WORDS[COMP_CWORD]}}"
    prev="${{COMP_WORDS[COMP_CWORD-1]}}"

    if [[ "${{cur}}" == -* ]]; then
        local options=(
            "--completion    (Generate shell completion script)"
            "-h              (Print help)"
            "--help          (Print help)"
            "-i              (List installed versions)"
            "--installed     (List installed versions)"
            "--insecure      (Skip TLS certificate verification)"
            "-k              (Skip TLS certificate verification)"
            "-l              (List available versions from the mirror)"
            "--list          (List available versions from the mirror)"
            "-m              (Download version matching connected cluster)"
            "--match-server  (Download version matching connected cluster)"
            "-p              (Remove installed versions)"
            "--prune         (Remove installed versions)"
            "-u              (Update ovc to latest version)"
            "--update        (Update ovc to latest version)"
            "-v              (Make the operation more talkative)"
            "--verbose       (Make the operation more talkative)"
            "--version       (Print version)"
        )

        local IFS=$'\n'
        local opt name padded
        local width=$((COLUMNS - 1))
        for opt in "${{options[@]}}"; do
            name="${{opt%%  *}}"
            if [[ "$name" == "${{cur}}"* ]]; then
                printf -v padded "%-${{width}}s" "$opt"
                COMPREPLY+=("$padded")
            fi
        done

        if ((${{#COMPREPLY[@]}} == 1)); then
            COMPREPLY[0]="${{COMPREPLY[0]%%  *}}"
        fi
    fi
}}

complete -o nosort -F _ovc_completions ovc
"#
    );
}
