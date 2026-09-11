// SPDX-License-Identifier: AGPL-3.0-or-later
//! Platform detection and download URL building for `oc` binaries.

/// Base URL of the OpenShift mirror hosting client binaries.
pub const OC_MIRROR_BASE: &str = "https://mirror.openshift.com/pub/openshift-v4";

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

    /// Build the download URL for a version on this platform.
    #[must_use]
    pub fn build_download_url(&self, version: &str) -> String {
        format!(
            "{}/{}/clients/ocp/{}/openshift-client-{}-{}.{}",
            OC_MIRROR_BASE,
            self.mirror_path,
            version,
            self.binary_suffix,
            version,
            self.file_extension
        )
    }

    /// Build the URL listing available versions for this platform.
    #[must_use]
    pub fn build_versions_url(&self) -> String {
        format!("{}/{}/clients/ocp/", OC_MIRROR_BASE, self.mirror_path)
    }
}
