// SPDX-License-Identifier: AGPL-3.0-or-later
//! Resolve XDG base directories, falling back to `HOME`.

use std::error::Error;
use std::path::PathBuf;

/// Variable naming the user cache directory.
const CACHE_VAR: &str = "XDG_CACHE_HOME";

/// Resolve an XDG base directory, else `HOME` plus a suffix.
///
/// # Errors
/// Fails when neither the variable nor HOME holds a path.
pub fn base_dir(var: &str, home_suffix: &str) -> Result<PathBuf, Box<dyn Error>> {
    if let Some(base) = env_path(var) {
        return Ok(base);
    }

    Ok(home_dir()?.join(home_suffix))
}

/// Resolve the ovc cache root directory.
///
/// # Errors
/// Fails when neither `XDG_CACHE_HOME` nor HOME holds a path.
pub fn cache_root() -> Result<PathBuf, Box<dyn Error>> {
    if let Some(base) = env_path(CACHE_VAR) {
        return Ok(base.join("ovc"));
    }

    Ok(home_dir()?.join(".ovc"))
}

/// Warn when the cache falls back to `~/.ovc`.
pub fn warn_cache_fallback(verbose: bool) {
    if verbose && env_path(CACHE_VAR).is_none() {
        eprintln!("Warning: XDG_CACHE_HOME unset, using ~/.ovc");
    }
}

/// Read a variable as a path, rejecting an empty value.
fn env_path(var: &str) -> Option<PathBuf> {
    // Empty variable would yield relative path
    std::env::var(var)
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

/// Read HOME as a path, rejecting an empty value.
fn home_dir() -> Result<PathBuf, Box<dyn Error>> {
    env_path("HOME").ok_or_else(|| "HOME is not set".into())
}
