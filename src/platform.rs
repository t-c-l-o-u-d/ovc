// GNU Affero General Public License v3.0 or later (see LICENSE or https://www.gnu.org/licenses/agpl.txt)
//! Platform detection and URL building for OpenShift client binaries
//!
//! This module provides functionality for detecting the current platform and
//! building appropriate download URLs for OpenShift client binaries from the
//! official mirror.

/// Base URL for the OpenShift mirror where client binaries are hosted
pub const OC_MIRROR_BASE: &str = "https://mirror.openshift.com/pub/openshift-v4";

/// Default directory to store downloaded oc binaries relative to user's home directory
pub const OC_BIN_DIR: &str = ".local/bin/oc_bins";

/// Represents a target platform for OpenShift client binaries
///
/// Each platform defines the specific paths and naming conventions used
/// by the OpenShift mirror for that platform's binaries.
#[derive(Debug, Clone)]
pub struct Platform {
    /// Human-readable platform name (e.g. "linux-x86_64")
    pub name: &'static str,
    /// Mirror subdirectory path for this platform
    pub mirror_path: &'static str,
    /// Binary suffix used in download URLs
    pub binary_suffix: &'static str,
    /// File extension for the downloaded archive
    pub file_extension: &'static str,
}

impl Platform {
    /// Linux x86_64 platform configuration
    pub const LINUX_X86_64: Platform = Platform {
        name: "linux-x86_64",
        mirror_path: "x86_64",
        binary_suffix: "linux",
        file_extension: "tar.gz",
    };

    /// Linux ARM64 platform configuration
    pub const LINUX_ARM64: Platform = Platform {
        name: "linux-arm64",
        mirror_path: "aarch64",
        binary_suffix: "linux",
        file_extension: "tar.gz",
    };

    /// macOS x86_64 platform configuration
    pub const MAC_X86_64: Platform = Platform {
        name: "mac-x86_64",
        mirror_path: "x86_64",
        binary_suffix: "mac",
        file_extension: "tar.gz",
    };

    /// macOS ARM64 platform configuration
    /// Note: Mac ARM64 binaries are stored in the x86_64 directory on the mirror
    pub const MAC_ARM64: Platform = Platform {
        name: "mac-arm64",
        mirror_path: "x86_64",
        binary_suffix: "mac-arm64",
        file_extension: "tar.gz",
    };

    /// Automatically detect the current platform based on OS and architecture
    ///
    /// Returns the appropriate Platform constant based on the runtime environment.
    /// Falls back to LINUX_X86_64 for unsupported platforms.
    pub fn detect() -> Platform {
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("linux", "x86_64" | "amd64") => Self::LINUX_X86_64,
            ("linux", "aarch64" | "arm64") => Self::LINUX_ARM64,
            ("macos", "x86_64" | "amd64") => Self::MAC_X86_64,
            ("macos", "aarch64" | "arm64") => Self::MAC_ARM64,
            // Default fallbacks for known OS with unknown architecture
            ("linux", _) => Self::LINUX_X86_64,
            ("macos", _) => Self::MAC_X86_64,
            // Ultimate fallback for unknown OS
            _ => Self::LINUX_X86_64,
        }
    }

    /// Build the download URL for a specific version on this platform
    ///
    /// # Arguments
    /// * `version` - The OpenShift version to download (e.g. "4.19.0")
    ///
    /// # Returns
    /// Complete URL to download the specified version for this platform
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

    /// Build the base URL for listing available versions on this platform
    ///
    /// # Returns
    /// URL to the directory listing of available versions for this platform
    pub fn build_versions_url(&self) -> String {
        format!("{}/{}/clients/ocp/", OC_MIRROR_BASE, self.mirror_path)
    }
}
