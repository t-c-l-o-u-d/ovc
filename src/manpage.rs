// GNU Affero General Public License v3.0 or later (see LICENSE or https://www.gnu.org/licenses/agpl.txt)
// Man page self-installation
//
// Fetches the man page from GitHub on first run of each version and installs
// it to $XDG_DATA_HOME/man/man1/ovc.1 so that `man ovc` works without a
// package manager.

use std::error::Error;
use std::fs;
use std::path::PathBuf;

const GITHUB_OWNER: &str = "t-c-l-o-u-d";
const GITHUB_REPO: &str = "ovc";

/// Get the ovc data directory under XDG_DATA_HOME
///
/// Uses `$XDG_DATA_HOME` if set, otherwise falls back to `$HOME/.local/share`.
///
/// # Errors
/// Returns error if HOME environment variable is not set or directory creation fails
pub fn get_data_dir() -> Result<PathBuf, Box<dyn Error>> {
    let data_base = std::env::var("XDG_DATA_HOME")
        .or_else(|_| std::env::var("HOME").map(|home| format!("{home}/.local/share")))?;
    let data_dir = PathBuf::from(data_base).join("ovc");
    fs::create_dir_all(&data_dir)?;
    Ok(data_dir)
}

/// Get the man page installation directory
///
/// # Errors
/// Returns error if the directory cannot be created
pub fn get_man_install_dir() -> Result<PathBuf, Box<dyn Error>> {
    let data_base = std::env::var("XDG_DATA_HOME")
        .or_else(|_| std::env::var("HOME").map(|home| format!("{home}/.local/share")))?;
    let man_dir = PathBuf::from(data_base).join("man").join("man1");
    fs::create_dir_all(&man_dir)?;
    Ok(man_dir)
}

/// Get the path to the man-version tracking file
///
/// # Errors
/// Returns error if the data directory cannot be created
pub fn get_man_version_path() -> Result<PathBuf, Box<dyn Error>> {
    Ok(get_data_dir()?.join("man-version"))
}

/// Read the currently installed man page version
///
/// Returns `None` if the version file doesn't exist or can't be read.
#[must_use]
pub fn read_installed_version() -> Option<String> {
    let path = get_man_version_path().ok()?;
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

/// Fetch the man page content from GitHub for a specific version
fn fetch_man_page(version: &str, verbose: bool) -> Result<String, Box<dyn Error>> {
    let url = format!(
        "https://raw.githubusercontent.com/{GITHUB_OWNER}/{GITHUB_REPO}/v{version}/man/ovc.1"
    );

    if verbose {
        eprintln!("Fetching man page from: {url}");
    }

    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("ovc/{}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let resp = client.get(&url).send()?;

    if !resp.status().is_success() {
        return Err(format!("Failed to fetch man page: {} ({})", url, resp.status()).into());
    }

    Ok(resp.text()?)
}

/// Write the man page file to the installation directory
fn write_man_page(content: &str) -> Result<(), Box<dyn Error>> {
    let man_dir = get_man_install_dir()?;
    fs::write(man_dir.join("ovc.1"), content)?;
    Ok(())
}

/// Write the version tracking file
///
/// # Errors
/// Returns error if the data directory or file write fails
pub fn write_version_file(version: &str) -> Result<(), Box<dyn Error>> {
    let path = get_man_version_path()?;
    fs::write(path, version)?;
    Ok(())
}

/// Install the man page for a specific version
///
/// Fetches from GitHub and writes to the local man directory.
///
/// # Errors
/// Returns error if the fetch or file write fails
pub fn install_man_page_for_version(version: &str, verbose: bool) -> Result<(), Box<dyn Error>> {
    let content = fetch_man_page(version, verbose)?;
    write_man_page(&content)?;
    write_version_file(version)?;

    if verbose {
        eprintln!("Installed man page for version {version}");
    }

    Ok(())
}

/// Ensure the man page is installed for the current binary version
///
/// Compares the installed man page version with `CARGO_PKG_VERSION`.
/// If they differ, fetches and installs the matching man page.
/// All failures are silent unless verbose mode is enabled.
pub fn ensure_man_page(verbose: bool) {
    let current_version = env!("CARGO_PKG_VERSION");

    if let Some(installed) = read_installed_version()
        && installed == current_version
    {
        return;
    }

    if let Err(e) = install_man_page_for_version(current_version, verbose)
        && verbose
    {
        eprintln!("Warning: failed to install man page: {e}");
    }
}
