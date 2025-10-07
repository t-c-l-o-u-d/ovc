// GNU Affero General Public License v3.0 or later (see LICENSE or https://www.gnu.org/licenses/agpl.txt)
//! Version comparison and utility functions for OpenShift client versions
//!
//! This module provides utilities for parsing, comparing, and manipulating
//! version strings, including support for semantic versioning and pre-release
//! versions.

use std::path::Path;

/// Compare two version strings using semantic versioning rules
///
/// Handles both stable versions (e.g. "4.19.0") and pre-release versions
/// (e.g. "4.19.0-rc.1"). Pre-release versions are considered less than
/// their corresponding stable versions.
///
/// # Arguments
/// * `a` - First version string to compare
/// * `b` - Second version string to compare
///
/// # Returns
/// `std::cmp::Ordering` indicating the relationship between the versions
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse_version = |v: &str| -> (Vec<u32>, bool, String) {
        // Split on '-' to separate base version from pre-release suffix
        let parts: Vec<&str> = v.split('-').collect();
        let base_version = parts[0];
        let is_prerelease = parts.len() > 1;
        let prerelease_suffix = if is_prerelease {
            parts[1..].join("-")
        } else {
            String::new()
        };

        let version_parts: Vec<u32> = base_version
            .split('.')
            .filter_map(|part| {
                // Extract only the numeric part, ignoring non-numeric suffixes like "EUS"
                let numeric_part = part.split_whitespace().next().unwrap_or(part);
                numeric_part.parse::<u32>().ok()
            })
            .collect();

        (version_parts, is_prerelease, prerelease_suffix)
    };

    let (a_parts, a_is_prerelease, a_suffix) = parse_version(a);
    let (b_parts, b_is_prerelease, b_suffix) = parse_version(b);

    // Compare base version parts numerically
    let max_len = a_parts.len().max(b_parts.len());
    for i in 0..max_len {
        let a_part = a_parts.get(i).unwrap_or(&0);
        let b_part = b_parts.get(i).unwrap_or(&0);
        match a_part.cmp(b_part) {
            std::cmp::Ordering::Equal => {}
            other => return other,
        }
    }

    // If base versions are equal, handle pre-release logic
    match (a_is_prerelease, b_is_prerelease) {
        (true, false) => std::cmp::Ordering::Less, // 4.19.0-rc.1 < 4.19.0
        (false, true) => std::cmp::Ordering::Greater, // 4.19.0 > 4.19.0-rc.1
        (true, true) => a_suffix.cmp(&b_suffix),   // rc.1 vs rc.2
        (false, false) => a.cmp(b),                // fallback for suffixes like "EUS"
    }
}

/// Extract major.minor version from a full version string
///
/// # Arguments
/// * `version` - Full version string (e.g. "4.19.0" or "4.19.0-rc.1")
///
/// # Returns
/// `Some("major.minor")` if the version has at least major and minor components,
/// `None` otherwise
///
/// # Examples
/// ```
/// use ovc::version::extract_major_minor;
/// assert_eq!(extract_major_minor("4.19.0"), Some("4.19".to_string()));
/// assert_eq!(extract_major_minor("4.19.0-rc.1"), Some("4.19".to_string()));
/// assert_eq!(extract_major_minor("4"), None);
/// ```
pub fn extract_major_minor(version: &str) -> Option<String> {
    let mut parts = version.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;
    if !major.is_empty() && !minor.is_empty() {
        Some(format!("{major}.{minor}"))
    } else {
        None
    }
}

/// Extract version number from command output or version string
///
/// Finds the first sequence of digits and dots in the input string.
/// Useful for parsing version output from commands.
///
/// # Arguments
/// * `version_output` - String containing version information
///
/// # Returns
/// The first sequence of digits and dots found, or the original string if none found
///
/// # Examples
/// ```
/// use ovc::version::extract_version_number;
/// assert_eq!(extract_version_number("4.19.0"), "4.19.0");
/// assert_eq!(extract_version_number("4.19.0-dirty"), "4.19.0");
/// ```
pub fn extract_version_number(version_output: &str) -> &str {
    version_output
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .next()
        .unwrap_or(version_output)
}

/// Check if a version string represents a stable (non-pre-release) version
///
/// Returns `false` for versions containing pre-release indicators like
/// "rc", "alpha", "beta", "nightly", "dev", or "snapshot".
///
/// # Arguments
/// * `version` - Version string to check
///
/// # Returns
/// `true` if the version appears to be stable, `false` otherwise
///
/// # Examples
/// ```
/// use ovc::version::is_stable_version;
/// assert!(is_stable_version("4.19.0"));
/// assert!(!is_stable_version("4.19.0-rc.1"));
/// ```
pub fn is_stable_version(version: &str) -> bool {
    let version_lower = version.to_lowercase();
    !version_lower.contains("-rc")
        && !version_lower.contains("-alpha")
        && !version_lower.contains("-beta")
        && !version_lower.contains("-nightly")
        && !version_lower.contains("-dev")
        && !version_lower.contains("-snapshot")
}

/// Extract version string from a binary file path
///
/// Assumes the file is named "oc-{version}" and extracts the version part.
///
/// # Arguments
/// * `path` - Path to the oc binary file
///
/// # Returns
/// Version string extracted from the filename, or "unknown" if extraction fails
///
/// # Examples
/// ```
/// use std::path::Path;
/// use ovc::version::extract_version_from_path;
///
/// let path = Path::new("/path/to/oc-4.19.0");
/// assert_eq!(extract_version_from_path(path), "4.19.0");
/// ```
pub fn extract_version_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix("oc-"))
        .unwrap_or("unknown")
        .to_string()
}

/// Find the best matching version from available versions
///
/// First attempts an exact match. If not found, looks for the latest version
/// that matches the major.minor prefix of the requested version.
///
/// # Arguments
/// * `server_version` - Requested version (can be partial like "4.19")
/// * `available_versions` - List of available versions to search
///
/// # Returns
/// `Some(version)` if a match is found, `None` otherwise
///
/// # Examples
/// ```
/// use ovc::version::find_matching_version;
///
/// let available = vec!["4.19.0".to_string(), "4.19.1".to_string(), "4.20.0".to_string()];
/// assert_eq!(find_matching_version("4.19", &available), Some("4.19.1".to_string()));
/// assert_eq!(find_matching_version("4.19.0", &available), Some("4.19.0".to_string()));
/// ```
pub fn find_matching_version(
    server_version: &str,
    available_versions: &[String],
) -> Option<String> {
    // First try exact match
    if available_versions.contains(&server_version.to_string()) {
        return Some(server_version.to_string());
    }

    // Try to find the closest version by comparing major.minor.patch
    let server_parts: Vec<&str> = server_version.split('.').collect();
    if server_parts.len() < 2 {
        return None;
    }

    let server_major_minor = format!("{}.{}", server_parts[0], server_parts[1]);

    // Look for versions that match major.minor
    let mut candidates: Vec<String> = available_versions
        .iter()
        .filter(|v| v.starts_with(&server_major_minor))
        .cloned()
        .collect();

    if candidates.is_empty() {
        return None;
    }

    // Sort and return the latest matching version
    candidates.sort_by(|a, b| compare_versions(a, b));
    candidates.last().cloned()
}
