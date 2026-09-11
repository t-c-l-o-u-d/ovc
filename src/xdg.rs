// SPDX-License-Identifier: AGPL-3.0-or-later
//! Resolve XDG base directories, falling back to `HOME`.

use std::error::Error;
use std::path::PathBuf;

/// Resolve an XDG base directory, else `HOME` plus a suffix.
///
/// # Errors
/// Fails when neither the variable nor HOME holds a path.
pub fn base_dir(var: &str, home_suffix: &str) -> Result<PathBuf, Box<dyn Error>> {
    // Empty variable would yield relative path
    if let Some(base) = std::env::var(var).ok().filter(|s| !s.is_empty()) {
        return Ok(PathBuf::from(base));
    }

    let home = std::env::var("HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or("HOME is not set")?;

    Ok(PathBuf::from(home).join(home_suffix))
}
