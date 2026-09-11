// SPDX-License-Identifier: AGPL-3.0-or-later
//! Install the embedded man page under `XDG_DATA_HOME`.

use std::error::Error;
use std::fs;
use std::path::PathBuf;

/// Man page rendered by build.rs at compile time.
const MAN_PAGE: &str = include_str!(concat!(env!("OUT_DIR"), "/ovc.1"));

/// Resolve the base data directory.
fn data_base() -> Result<PathBuf, Box<dyn Error>> {
    crate::xdg::base_dir("XDG_DATA_HOME", ".local/share")
}

/// Get the ovc data directory, creating it when absent.
///
/// # Errors
/// Fails when HOME is unset or the directory cannot be created.
pub fn get_data_dir() -> Result<PathBuf, Box<dyn Error>> {
    let data_dir = data_base()?.join("ovc");
    fs::create_dir_all(&data_dir)?;
    Ok(data_dir)
}

/// Get the man page directory, creating it when absent.
///
/// # Errors
/// Fails when HOME is unset or the directory cannot be created.
pub fn get_man_install_dir() -> Result<PathBuf, Box<dyn Error>> {
    let man_dir = data_base()?.join("man").join("man1");
    fs::create_dir_all(&man_dir)?;
    Ok(man_dir)
}

/// Get the path of the version tracking file.
///
/// # Errors
/// Fails when the data directory cannot be created.
pub fn get_man_version_path() -> Result<PathBuf, Box<dyn Error>> {
    Ok(get_data_dir()?.join("man-version"))
}

/// Read the installed man page version, if recorded.
#[must_use]
pub fn read_installed_version() -> Option<String> {
    let path = get_man_version_path().ok()?;
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

/// Write the man page to the install directory.
fn write_man_page() -> Result<(), Box<dyn Error>> {
    let man_dir = get_man_install_dir()?;
    fs::write(man_dir.join("ovc.1"), MAN_PAGE)?;
    Ok(())
}

/// Record the installed man page version.
///
/// # Errors
/// Fails when the data directory or file write fails.
pub fn write_version_file(version: &str) -> Result<(), Box<dyn Error>> {
    let path = get_man_version_path()?;
    fs::write(path, version)?;
    Ok(())
}

/// Write the man page and record its version.
///
/// # Errors
/// Fails when either file cannot be written.
pub fn install_man_page(verbose: bool) -> Result<(), Box<dyn Error>> {
    let version = env!("CARGO_PKG_VERSION");
    write_man_page()?;
    write_version_file(version)?;

    if verbose {
        eprintln!("Installed man page for version {version}");
    }

    Ok(())
}

/// Reinstall the man page when its version is stale.
pub fn ensure_man_page(verbose: bool) {
    let current_version = env!("CARGO_PKG_VERSION");

    if let Some(installed) = read_installed_version()
        && installed == current_version
    {
        return;
    }

    if let Err(e) = install_man_page(verbose)
        && verbose
    {
        eprintln!("Warning: Cannot install man page: {e}");
    }
}
