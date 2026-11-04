// SPDX-License-Identifier: AGPL-3.0-or-later
//! Manage OpenShift client versions: platform detection, version comparison, caching.

pub mod cache;
pub mod manpage;
pub mod platform;
pub mod version;
pub mod xdg;

pub use platform::{MIRROR_MAJORS, MIRROR_ROOT, Platform};
pub use version::{
    compare_versions, find_matching_version, major_version, matches_version_pattern,
};
