// GNU Affero General Public License v3.0 or later (see LICENSE or https://www.gnu.org/licenses/agpl.txt)
// Allow multiple crate versions for Windows-only dependencies (we only target Linux)
#![allow(clippy::multiple_crate_versions)]
//! OpenShift Client Version Control Library
//!
//! This library provides functionality for managing OpenShift client (`oc`) versions,
//! including platform detection, version comparison, and utility functions for
//! downloading and organizing different versions of the OpenShift CLI tool.

// Re-export public API from organized modules
pub mod cache;
pub mod manpage;
pub mod platform;
pub mod version;

// Re-export commonly used items at the crate root for convenience
pub use platform::{OC_BIN_DIR, OC_MIRROR_BASE, Platform};
pub use version::{
    compare_versions, extract_major_minor, extract_version_from_path, extract_version_number,
    find_matching_version, is_stable_version, matches_version_pattern,
};
