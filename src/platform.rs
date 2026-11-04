// SPDX-License-Identifier: AGPL-3.0-or-later
//! Platform detection and download URL building for `oc` binaries.

use crate::major_version;

/// Root URL of the OpenShift mirror.
pub const MIRROR_ROOT: &str = "https://mirror.openshift.com/pub";

/// Major versions with an `openshift-v{major}` mirror tree.
pub const MIRROR_MAJORS: &[u32] = &[4, 5];

/// Target platform for an OpenShift client binary.
#[derive(Debug, Clone)]
pub struct Platform {
    /// Platform name, such as `linux-x86_64`.
    pub name: &'static str,
    /// Mirror subdirectory for this platform.
    pub mirror_path: &'static str,
    /// Binary suffix used in download URLs.
    pub binary_suffix: &'static str,
    /// Extension of the downloaded archive.
    pub file_extension: &'static str,
}

impl Platform {
    /// Linux `x86_64` platform configuration.
    pub const LINUX_X86_64: Platform = Platform {
        name: "linux-x86_64",
        mirror_path: "x86_64",
        binary_suffix: "linux",
        file_extension: "tar.gz",
    };

    /// Detect the current platform.
    #[must_use]
    pub fn detect() -> Platform {
        Self::LINUX_X86_64
    }

    /// Build a version's download URL, or `None` without a numeric major.
    #[must_use]
    pub fn build_download_url(&self, version: &str) -> Option<String> {
        let major = major_version(version)?;
        Some(format!(
            "{}{}/openshift-client-{}-{}.{}",
            self.build_versions_url(major),
            version,
            self.binary_suffix,
            version,
            self.file_extension
        ))
    }

    /// Build the URL listing one major's versions.
    #[must_use]
    pub fn build_versions_url(&self, major: u32) -> String {
        format!(
            "{MIRROR_ROOT}/openshift-v{major}/{}/clients/ocp/",
            self.mirror_path
        )
    }
}
