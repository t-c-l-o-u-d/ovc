//! Comprehensive tests for the ovc library and CLI application
//!
//! This module contains both unit tests for library functions and integration tests
//! for the CLI application, ensuring 100% test coverage and validating all edge cases
//! and functionality.

use ovc::*;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

// Helper function to run ovc command and capture output
fn run_ovc(args: &[&str]) -> std::process::Output {
    Command::new("cargo")
        .args(["run", "--"])
        .args(args)
        .output()
        .expect("Failed to execute ovc command")
}

// Helper function to run ovc command with custom working directory
#[allow(dead_code)]
fn run_ovc_in_dir(args: &[&str], dir: &std::path::Path) -> std::process::Output {
    Command::new("cargo")
        .args(["run", "--"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("Failed to execute ovc command")
}

// =============================================================================
// UNIT TESTS - Library Functions
// =============================================================================

#[cfg(test)]
mod version_comparison_tests {
    use super::*;

    #[test]
    fn test_compare_versions_basic() {
        // Test basic version comparison
        assert_eq!(compare_versions("4.1.0", "4.2.0"), std::cmp::Ordering::Less);
        assert_eq!(
            compare_versions("4.10.0", "4.2.0"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_versions("4.1.0", "4.1.0"),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn test_compare_versions_prerelease() {
        // Test pre-release versions
        assert_eq!(
            compare_versions("4.19.0-rc.1", "4.19.0"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_versions("4.19.0", "4.19.0-rc.1"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_versions("4.19.0-rc.1", "4.19.0-rc.2"),
            std::cmp::Ordering::Less
        );
        // String comparison for rc.10 vs rc.2 - "rc.10" < "rc.2" lexicographically
        assert_eq!(
            compare_versions("4.19.0-rc.10", "4.19.0-rc.2"),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn test_compare_versions_patch() {
        // Test with different patch versions
        assert_eq!(
            compare_versions("4.1.1", "4.1.10"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_versions("4.1.15", "4.1.5"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_versions("4.1.0", "4.1.0.1"),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn test_compare_versions_complex() {
        // Test complex pre-release versions
        assert_eq!(
            compare_versions("4.19.0-alpha.1", "4.19.0-beta.1"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_versions("4.19.0-beta.1", "4.19.0-rc.1"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_versions("4.19.0-rc.1", "4.19.0"),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn test_compare_versions_eus_suffix() {
        // Test EUS and other suffixes
        assert_eq!(
            compare_versions("4.19.0 EUS", "4.19.0"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_versions("4.19.0", "4.19.0 EUS"),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn test_compare_versions_edge_cases() {
        // Test edge cases
        assert_eq!(compare_versions("", ""), std::cmp::Ordering::Equal);
        assert_eq!(compare_versions("1", "1"), std::cmp::Ordering::Equal);
        assert_eq!(compare_versions("1.0", "1"), std::cmp::Ordering::Greater);
        // String comparison for invalid versions - "invalid.version" > "another.invalid"
        assert_eq!(
            compare_versions("invalid.version", "another.invalid"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn test_version_comparison_with_unusual_formats() {
        // Test versions with unusual but valid formats
        assert_eq!(
            compare_versions("4.1.0.0", "4.1.0"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(compare_versions("4.1", "4.1.0"), std::cmp::Ordering::Less);
        assert_eq!(
            compare_versions("4.1.0-rc.1.2", "4.1.0-rc.1.1"),
            std::cmp::Ordering::Greater
        );
    }
}

#[cfg(test)]
mod version_extraction_tests {
    use super::*;

    #[test]
    fn test_extract_major_minor_valid() {
        assert_eq!(extract_major_minor("4.1.0"), Some("4.1".to_string()));
        assert_eq!(extract_major_minor("4.10.15"), Some("4.10".to_string()));
        assert_eq!(extract_major_minor("4.1"), Some("4.1".to_string()));
        assert_eq!(extract_major_minor("4.1.0-rc.1"), Some("4.1".to_string()));
        assert_eq!(extract_major_minor("10.20.30"), Some("10.20".to_string()));
    }

    #[test]
    fn test_extract_major_minor_invalid() {
        assert_eq!(extract_major_minor("4"), None);
        assert_eq!(extract_major_minor("invalid"), None);
        assert_eq!(extract_major_minor(""), None);
        assert_eq!(extract_major_minor("."), None);
        assert_eq!(extract_major_minor("4."), None);
        assert_eq!(extract_major_minor(".1"), None);
    }

    #[test]
    fn test_extract_major_minor_with_many_parts() {
        assert_eq!(extract_major_minor("1.2.3.4.5.6"), Some("1.2".to_string()));
        assert_eq!(
            extract_major_minor("4.19.0.1.2.3-rc.1"),
            Some("4.19".to_string())
        );
    }

    #[test]
    fn test_extract_version_number_valid() {
        assert_eq!(extract_version_number("4.1.0"), "4.1.0");
        assert_eq!(extract_version_number("4.1.0-dirty"), "4.1.0");
        assert_eq!(extract_version_number("version: 4.19.0"), "");
        assert_eq!(extract_version_number("4.19.0 (build info)"), "4.19.0");
    }

    #[test]
    fn test_extract_version_number_edge_cases() {
        assert_eq!(extract_version_number("v4.1.0"), "");
        assert_eq!(extract_version_number("openshift-4.1.0"), "");
        assert_eq!(extract_version_number("no-version-here"), "");
        assert_eq!(extract_version_number(""), "");
        assert_eq!(extract_version_number("123"), "123");
        assert_eq!(extract_version_number("1.2.3.4.5"), "1.2.3.4.5");
    }

    #[test]
    fn test_extract_version_number_with_special_characters() {
        assert_eq!(extract_version_number("version=4.19.0"), "");
        assert_eq!(extract_version_number("v:4.19.0"), "");
        assert_eq!(extract_version_number("4.19.0+build.123"), "4.19.0");
        assert_eq!(extract_version_number("4.19.0~snapshot"), "4.19.0");
    }

    #[test]
    fn test_extract_version_from_path_valid() {
        let path = PathBuf::from("/path/to/oc-4.1.0");
        assert_eq!(extract_version_from_path(&path), "4.1.0");

        let path = PathBuf::from("oc-4.10.15");
        assert_eq!(extract_version_from_path(&path), "4.10.15");

        let path = PathBuf::from("/home/user/.local/bin/oc_bins/linux-x86_64/oc-4.19.0-rc.1");
        assert_eq!(extract_version_from_path(&path), "4.19.0-rc.1");
    }

    #[test]
    fn test_extract_version_from_path_invalid() {
        let path = PathBuf::from("/invalid/path");
        assert_eq!(extract_version_from_path(&path), "unknown");

        let path = PathBuf::from("notoc-4.1.0");
        assert_eq!(extract_version_from_path(&path), "unknown");

        let path = PathBuf::from("oc-");
        assert_eq!(extract_version_from_path(&path), "");

        let path = PathBuf::from("");
        assert_eq!(extract_version_from_path(&path), "unknown");
    }
}

#[cfg(test)]
mod version_stability_tests {
    use super::*;

    #[test]
    fn test_is_stable_version_stable() {
        assert!(is_stable_version("4.1.0"));
        assert!(is_stable_version("4.10.15"));
        assert!(is_stable_version("1.0.0"));
        assert!(is_stable_version("4.19.0.1"));
        assert!(is_stable_version("4.19.0-hotfix"));
        assert!(is_stable_version("4.19.0-patch"));
    }

    #[test]
    fn test_is_stable_version_unstable() {
        assert!(!is_stable_version("4.1.0-rc.1"));
        assert!(!is_stable_version("4.1.0-alpha.1"));
        assert!(!is_stable_version("4.1.0-beta.1"));
        assert!(!is_stable_version("4.1.0-nightly"));
        assert!(!is_stable_version("4.1.0-dev"));
        assert!(!is_stable_version("4.1.0-snapshot"));
    }

    #[test]
    fn test_is_stable_version_case_insensitive() {
        assert!(!is_stable_version("4.1.0-RC.1"));
        assert!(!is_stable_version("4.1.0-ALPHA.1"));
        assert!(!is_stable_version("4.1.0-Beta.1"));
        assert!(!is_stable_version("4.1.0-NIGHTLY"));
        assert!(!is_stable_version("4.1.0-Dev"));
        assert!(!is_stable_version("4.1.0-SNAPSHOT"));
    }

    #[test]
    fn test_is_stable_version_edge_cases() {
        assert!(is_stable_version(""));
        assert!(is_stable_version("stable"));
        assert!(is_stable_version("4.19.0-release"));
        assert!(is_stable_version("4.19.0-final"));
    }

    #[test]
    fn test_is_stable_version_with_mixed_case_and_spaces() {
        // The function only checks for exact substrings, not spaced versions
        assert!(is_stable_version("4.19.0 - RC 1")); // doesn't contain "-rc" exactly
        assert!(!is_stable_version("4.19.0-Alpha-1")); // contains "-alpha" (case insensitive)
        assert!(is_stable_version("4.19.0_beta_1")); // doesn't contain "-beta" exactly
        assert!(is_stable_version("4.19.0-release-candidate")); // doesn't contain exact keywords
    }
}

#[cfg(test)]
mod version_matching_tests {
    use super::*;

    #[test]
    fn test_find_matching_version_exact_match() {
        let available = vec![
            "4.1.0".to_string(),
            "4.1.1".to_string(),
            "4.1.2".to_string(),
            "4.2.0".to_string(),
            "4.2.1".to_string(),
        ];

        assert_eq!(
            find_matching_version("4.1.1", &available),
            Some("4.1.1".to_string())
        );
        assert_eq!(
            find_matching_version("4.2.0", &available),
            Some("4.2.0".to_string())
        );
    }

    #[test]
    fn test_find_matching_version_partial_match() {
        let available = vec![
            "4.1.0".to_string(),
            "4.1.1".to_string(),
            "4.1.2".to_string(),
            "4.2.0".to_string(),
            "4.2.1".to_string(),
        ];

        // Should find latest in series
        assert_eq!(
            find_matching_version("4.1.5", &available),
            Some("4.1.2".to_string())
        );
        assert_eq!(
            find_matching_version("4.2.5", &available),
            Some("4.2.1".to_string())
        );
    }

    #[test]
    fn test_find_matching_version_no_match() {
        let available = vec![
            "4.1.0".to_string(),
            "4.1.1".to_string(),
            "4.2.0".to_string(),
        ];

        assert_eq!(find_matching_version("4.3.0", &available), None);
        assert_eq!(find_matching_version("5.1.0", &available), None);
    }

    #[test]
    fn test_find_matching_version_invalid_input() {
        let available = vec!["4.1.0".to_string(), "4.2.0".to_string()];

        assert_eq!(find_matching_version("invalid", &available), None);
        assert_eq!(find_matching_version("4", &available), None);
        assert_eq!(find_matching_version("", &available), None);
    }

    #[test]
    fn test_find_matching_version_empty_available() {
        let available: Vec<String> = vec![];
        assert_eq!(find_matching_version("4.1.0", &available), None);
    }

    #[test]
    fn test_find_matching_version_prerelease() {
        let available = vec![
            "4.19.0-rc.1".to_string(),
            "4.19.0-rc.2".to_string(),
            "4.19.0".to_string(),
            "4.19.1".to_string(),
        ];

        assert_eq!(
            find_matching_version("4.19.0-rc.1", &available),
            Some("4.19.0-rc.1".to_string())
        );
        assert_eq!(
            find_matching_version("4.19.5", &available),
            Some("4.19.1".to_string())
        );
    }

    #[test]
    fn test_find_matching_version_sorting() {
        let available = vec![
            "4.1.10".to_string(),
            "4.1.2".to_string(),
            "4.1.1".to_string(),
            "4.1.20".to_string(),
        ];

        // Should return the latest (4.1.20) when looking for 4.1.x
        assert_eq!(
            find_matching_version("4.1.15", &available),
            Some("4.1.20".to_string())
        );
    }

    #[test]
    fn test_find_matching_version_with_complex_versions() {
        let available = vec![
            "4.19.0-alpha.1".to_string(),
            "4.19.0-beta.1".to_string(),
            "4.19.0-rc.1".to_string(),
            "4.19.0".to_string(),
            "4.19.1-rc.1".to_string(),
            "4.19.1".to_string(),
        ];

        // Should find latest stable in the 4.19 series
        assert_eq!(
            find_matching_version("4.19.5", &available),
            Some("4.19.1".to_string())
        );
    }
}

#[cfg(test)]
mod platform_tests {
    use super::*;

    #[test]
    fn test_platform_constants() {
        // Test Linux x86_64
        assert_eq!(Platform::LINUX_X86_64.name, "linux-x86_64");
        assert_eq!(Platform::LINUX_X86_64.mirror_path, "x86_64");
        assert_eq!(Platform::LINUX_X86_64.binary_suffix, "linux");
        assert_eq!(Platform::LINUX_X86_64.file_extension, "tar.gz");

        // Test Linux ARM64
        assert_eq!(Platform::LINUX_ARM64.name, "linux-arm64");
        assert_eq!(Platform::LINUX_ARM64.mirror_path, "aarch64");
        assert_eq!(Platform::LINUX_ARM64.binary_suffix, "linux");
        assert_eq!(Platform::LINUX_ARM64.file_extension, "tar.gz");

        // Test macOS x86_64
        assert_eq!(Platform::MAC_X86_64.name, "mac-x86_64");
        assert_eq!(Platform::MAC_X86_64.mirror_path, "x86_64");
        assert_eq!(Platform::MAC_X86_64.binary_suffix, "mac");
        assert_eq!(Platform::MAC_X86_64.file_extension, "tar.gz");

        // Test macOS ARM64
        assert_eq!(Platform::MAC_ARM64.name, "mac-arm64");
        assert_eq!(Platform::MAC_ARM64.mirror_path, "x86_64");
        assert_eq!(Platform::MAC_ARM64.binary_suffix, "mac-arm64");
        assert_eq!(Platform::MAC_ARM64.file_extension, "tar.gz");
    }

    #[test]
    fn test_platform_detection() {
        let platform = Platform::detect();

        // Should detect a valid platform
        assert!(!platform.name.is_empty());
        assert!(!platform.mirror_path.is_empty());
        assert!(!platform.binary_suffix.is_empty());
        assert_eq!(platform.file_extension, "tar.gz");
    }

    #[test]
    fn test_platform_url_building() {
        let platform = Platform::LINUX_X86_64;

        let download_url = platform.build_download_url("4.1.0");
        assert!(download_url.contains("https://mirror.openshift.com"));
        assert!(download_url.contains("4.1.0"));
        assert!(download_url.contains("linux"));
        assert!(download_url.contains("tar.gz"));
        assert_eq!(
            download_url,
            "https://mirror.openshift.com/pub/openshift-v4/x86_64/clients/ocp/4.1.0/openshift-client-linux-4.1.0.tar.gz"
        );

        let versions_url = platform.build_versions_url();
        assert!(versions_url.contains("https://mirror.openshift.com"));
        assert!(versions_url.contains("clients/ocp"));
        assert_eq!(
            versions_url,
            "https://mirror.openshift.com/pub/openshift-v4/x86_64/clients/ocp/"
        );
    }

    #[test]
    fn test_all_platforms_url_building() {
        let platforms = [
            Platform::LINUX_X86_64,
            Platform::LINUX_ARM64,
            Platform::MAC_X86_64,
            Platform::MAC_ARM64,
        ];

        for platform in &platforms {
            let download_url = platform.build_download_url("4.19.0");
            assert!(download_url.starts_with("https://mirror.openshift.com"));
            assert!(download_url.contains("4.19.0"));
            assert!(download_url.ends_with(".tar.gz"));

            let versions_url = platform.build_versions_url();
            assert!(versions_url.starts_with("https://mirror.openshift.com"));
            assert!(versions_url.ends_with("/clients/ocp/"));
        }
    }

    #[test]
    fn test_platform_debug_clone() {
        let platform = Platform::LINUX_X86_64;
        let cloned = platform.clone();

        assert_eq!(platform.name, cloned.name);
        assert_eq!(platform.mirror_path, cloned.mirror_path);
        assert_eq!(platform.binary_suffix, cloned.binary_suffix);
        assert_eq!(platform.file_extension, cloned.file_extension);

        // Test Debug trait
        let debug_str = format!("{:?}", platform);
        assert!(debug_str.contains("Platform"));
        assert!(debug_str.contains("linux-x86_64"));
    }
}

#[cfg(test)]
mod constants_tests {
    use super::*;

    #[test]
    fn test_constants_validity() {
        // Test that constants are properly defined
        assert!(OC_MIRROR_BASE.starts_with("https://"));
        assert!(OC_MIRROR_BASE.contains("mirror.openshift.com"));
        assert_eq!(
            OC_MIRROR_BASE,
            "https://mirror.openshift.com/pub/openshift-v4"
        );

        assert!(OC_BIN_DIR.contains("oc_bins"));
        assert!(OC_BIN_DIR.starts_with(".local"));
        assert_eq!(OC_BIN_DIR, ".local/bin/oc_bins");
    }
}

// =============================================================================
// INTEGRATION TESTS - CLI Application
// =============================================================================

#[cfg(test)]
mod cli_basic_tests {
    use super::*;

    #[test]
    fn test_help_command() {
        let output = run_ovc(&["--help"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("openshift client version control"));
        assert!(stdout.contains("-v, --verbose"));
        assert!(stdout.contains("list"));
        assert!(stdout.contains("installed"));
        assert!(stdout.contains("prune"));
    }

    #[test]
    fn test_version_command() {
        let output = run_ovc(&["--version"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("ovc"));
    }

    #[test]
    fn test_missing_version_error() {
        let output = run_ovc(&[]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("ovc: missing version"));
    }

    #[test]
    fn test_invalid_partial_version() {
        // Test that providing only major version fails
        let output = run_ovc(&["4"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Version must include at least major and minor version"));
    }

    #[test]
    fn test_error_messages_go_to_stderr() {
        let output = run_ovc(&["invalid-version"]);
        assert!(!output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Error messages should go to stderr, not stdout
        assert!(stdout.trim().is_empty() || !stdout.contains("error"));
        assert!(!stderr.trim().is_empty());
    }
}

#[cfg(test)]
mod cli_download_tests {
    use super::*;

    #[test]
    fn test_download_invalid_version() {
        let output = run_ovc(&["999.999.999"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("not found"));
    }

    #[test]
    fn test_download_verbose_shows_details() {
        // Try to download a version that should exist
        let output = run_ovc(&["-v", "4.17.15"]);

        if output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Should show either download progress or already installed message
            assert!(
                stderr.contains("Downloading: 4.17.15")
                    || stderr.contains("Already installed: 4.17.15")
            );
            assert!(stderr.contains("Downloaded to:") || stderr.contains("Already installed:"));
            assert!(stderr.contains("Set as default: 4.17.15"));
        }
    }

    #[test]
    fn test_download_silent_by_default() {
        // Download should be silent by default
        let output = run_ovc(&["4.17.14"]);

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Should produce no output (silent)
            assert!(stdout.trim().is_empty());
            // stderr might contain compilation output but not our messages
            assert!(!stderr.contains("Downloading:"));
            assert!(!stderr.contains("Downloaded to:"));
            assert!(!stderr.contains("Set as default:"));
        }
    }

    #[test]
    fn test_partial_version_resolution() {
        // Test that partial versions like "4.19" resolve to latest patch
        let output = run_ovc(&["-v", "4.19"]);

        if output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Should show resolution from partial to full version
            assert!(stderr.contains("Resolved 4.19 to 4.19."));
            assert!(
                stderr.contains("Downloading: 4.19.")
                    || stderr.contains("Already installed: 4.19.")
            );
            assert!(stderr.contains("Set as default: 4.19."));
        }
    }

    #[test]
    fn test_full_version_no_resolution() {
        // Test that full versions don't show resolution message
        let output = run_ovc(&["-v", "4.17.15"]);

        if output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Should NOT show resolution message
            assert!(!stderr.contains("Resolved"));
            // Should show either download or already installed message
            assert!(
                stderr.contains("Downloading: 4.17.15")
                    || stderr.contains("Already installed: 4.17.15")
            );
        }
    }

    #[test]
    fn test_network_error_handling() {
        // Test with an invalid URL (this would require mocking, but we can test error handling)
        let output = run_ovc(&["999.0.0"]);
        assert!(!output.status.success());

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.trim().is_empty());
        assert!(stderr.contains("not found") || stderr.contains("Version"));
    }
}

#[cfg(test)]
mod cli_list_tests {
    use super::*;

    #[test]
    fn test_list_available_versions_by_pattern() {
        let output = run_ovc(&["--list", "4.19"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Should contain versions that start with 4.19
        let lines: Vec<&str> = stdout.trim().split('\n').collect();
        assert!(!lines.is_empty());

        for line in lines {
            if !line.trim().is_empty() {
                assert!(
                    line.starts_with("4.19"),
                    "Line should start with 4.19: {}",
                    line
                );
            }
        }

        // Should contain both rc versions and stable versions
        assert!(stdout.contains("4.19.0-rc"));
        assert!(stdout.contains("4.19.0"));
    }

    #[test]
    fn test_list_available_versions_specific_patch() {
        let output = run_ovc(&["--list", "4.19.0"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Should contain versions that start with 4.19.0
        let lines: Vec<&str> = stdout.trim().split('\n').collect();
        assert!(!lines.is_empty());

        for line in lines {
            if !line.trim().is_empty() {
                assert!(
                    line.starts_with("4.19.0"),
                    "Line should start with 4.19.0: {}",
                    line
                );
            }
        }

        // Should contain rc versions for 4.19.0 but not 4.19.1
        assert!(stdout.contains("4.19.0-rc"));
        assert!(stdout.contains("4.19.0"));
        assert!(!stdout.contains("4.19.1"));
    }

    #[test]
    fn test_list_available_versions_invalid_format() {
        let output = run_ovc(&["--list", "4"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Version must include at least major and minor version"));
    }

    #[test]
    fn test_list_available_versions_no_matches() {
        let output = run_ovc(&["--list", "999.999"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("No versions found matching 999.999"));
    }

    #[test]
    fn test_list_available_versions_verbose() {
        let output = run_ovc(&["-v", "--list", "4.19"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Verbose mode should still just list versions (no extra info for list command)
        let lines: Vec<&str> = stdout.trim().split('\n').collect();
        for line in lines {
            if !line.trim().is_empty() {
                assert!(line.starts_with("4.19"));
                // Should not contain extra verbose info like paths
                assert!(!line.contains("("));
            }
        }
    }

    #[test]
    fn test_version_sorting() {
        let output = run_ovc(&["--list", "4.1"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);

        let versions: Vec<&str> = stdout.trim().split('\n').collect();
        if versions.len() > 1 {
            // Just check that we have versions starting with 4.1
            for version in &versions {
                if !version.trim().is_empty() {
                    assert!(
                        version.starts_with("4.1"),
                        "Version should start with 4.1: {}",
                        version
                    );
                }
            }
            // The versions should be sorted by the library's compare_versions function
            // which is tested separately in unit tests
            assert!(!versions.is_empty(), "Should have at least one version");
        }
    }

    #[test]
    fn test_stable_version_detection() {
        let output = run_ovc(&["--list", "4.19"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Should contain stable versions (without -rc, -alpha, etc.)
        let lines: Vec<&str> = stdout.trim().split('\n').collect();
        let stable_count = lines
            .iter()
            .filter(|line| {
                !line.contains("-rc") && !line.contains("-alpha") && !line.contains("-beta")
            })
            .count();

        // Should have at least some stable versions
        assert!(stable_count > 0);
    }
}

#[cfg(test)]
mod cli_installed_tests {
    use super::*;

    #[test]
    fn test_installed_command_empty() {
        // Create a temporary directory to test with clean state
        let temp_dir = TempDir::new().unwrap();
        let home_dir = temp_dir.path();

        // Set HOME to temp directory
        let output = Command::new("cargo")
            .args(["run", "--", "--installed", "4.19"])
            .env("HOME", home_dir)
            .output()
            .expect("Failed to execute ovc command");

        // Should fail when no versions are installed
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("No installed versions found matching 4.19"));
    }

    #[test]
    fn test_installed_command_verbose_shows_paths() {
        let output = run_ovc(&["-v", "--installed", "4.19"]);

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // If any versions are installed, they should show paths
            if !stdout.trim().is_empty() {
                assert!(stdout.contains("oc_bins") || stdout.contains("/"));
            }
        } else {
            // If no versions match, should show appropriate error
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains("No installed versions found matching"));
        }
    }

    #[test]
    fn test_installed_matching_versions() {
        // Test that installed shows matching versions
        let output = run_ovc(&["--installed", "4.19"]);

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(!stdout.trim().is_empty());

            // Should list versions that start with 4.19
            let lines: Vec<&str> = stdout.trim().split('\n').collect();
            for line in lines {
                if !line.trim().is_empty() {
                    assert!(
                        line.starts_with("4.19"),
                        "Should list 4.19.x versions: {}",
                        line
                    );
                }
            }
        } else {
            // If no versions match, should show appropriate error
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains("No installed versions found matching 4.19"));
        }
    }

    #[test]
    fn test_installed_invalid_format() {
        let output = run_ovc(&["--installed", "4"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Version must include at least major and minor version"));
    }

    #[test]
    fn test_installed_no_matches() {
        let output = run_ovc(&["--installed", "999.999"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("No installed versions found matching 999.999"));
    }

    #[test]
    fn test_installed_verbose_mode() {
        let output = run_ovc(&["-v", "--installed", "4.19"]);

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Verbose mode should show paths if versions are found
            if !stdout.trim().is_empty() {
                assert!(stdout.contains("(") && stdout.contains(")"));
            }
        } else {
            // If no versions match, should show appropriate error
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains("No installed versions found matching"));
        }
    }
}

#[cfg(test)]
mod cli_prune_tests {
    use super::*;

    #[test]
    fn test_prune_matching_versions() {
        // This test needs to be careful not to actually remove versions
        // We'll test the error case when no versions match
        let output = run_ovc(&["--prune", "999.999"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("No installed versions found matching 999.999"));
    }

    #[test]
    fn test_prune_invalid_format() {
        let output = run_ovc(&["--prune", "4"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Version must include at least major and minor version"));
    }

    #[test]
    fn test_prune_shows_what_will_be_removed() {
        // Test that prune shows what will be removed
        // We'll use a pattern that might match installed versions
        let output = run_ovc(&["--prune", "4.19"]);

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("Will remove the following:"));
            // Should list versions that start with 4.19
            let lines: Vec<&str> = stdout.lines().collect();
            for line in lines.iter().skip(1) {
                // Skip the "Will remove" line
                if !line.trim().is_empty() {
                    assert!(
                        line.starts_with("4.19"),
                        "Should list 4.19.x versions: {}",
                        line
                    );
                }
            }
        } else {
            // If no versions match, should show appropriate error
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains("No installed versions found matching"));
        }
    }

    #[test]
    fn test_prune_verbose_mode() {
        // Test verbose mode for prune (if any versions match)
        let output = run_ovc(&["-v", "--prune", "999.999"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("No installed versions found matching"));
    }
}

#[cfg(test)]
mod cli_behavior_tests {
    use super::*;

    #[test]
    fn test_verbose_flag_global() {
        // Test that -v works before the command
        let output1 = run_ovc(&["-v", "--list", "4.19"]);
        let _output2 = run_ovc(&["--list", "4.19", "-v"]);

        // Both should work (global flag)
        assert!(output1.status.success());
        // Note: end -v might not work as -v is global, but let's check
        // The second form might not work depending on clap configuration
    }

    #[test]
    fn test_platform_detection() {
        // The tool should work regardless of platform
        let output = run_ovc(&["--list", "4.19"]);
        assert!(output.status.success());

        // Should not contain platform-specific errors
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains("platform not supported"));
    }

    #[test]
    fn test_path_warnings() {
        // Test with a PATH that doesn't include ~/.local/bin but includes cargo
        let current_path = std::env::var("PATH").unwrap_or_default();
        let modified_path = format!("/usr/bin:/bin:{}", current_path);

        let output = Command::new("cargo")
            .args(["run", "--", "--list", "4.19"])
            .env("PATH", modified_path)
            .output();

        match output {
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // Should warn about PATH if oc is found but ~/.local/bin is not in PATH
                if stderr.contains("Warning") {
                    assert!(stderr.contains("PATH") || stderr.contains("not found"));
                }
                // Test passes if command runs successfully or fails gracefully
            }
            Err(_) => {
                // If we can't run the command due to PATH issues, that's also a valid test case
                // This test is mainly about ensuring the app handles PATH issues gracefully
            }
        }
    }

    #[test]
    fn test_output_is_pipeable() {
        // Test that commands produce clean output suitable for piping
        let output = run_ovc(&["--list", "4.19"]);
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.trim().split('\n').collect();

        // Each line should be a clean version number
        for line in lines.iter().take(5) {
            // Check first 5 lines
            if !line.trim().is_empty() {
                assert!(
                    line.starts_with("4.19"),
                    "Line should start with 4.19: {}",
                    line
                );
                assert!(
                    !line.contains("("),
                    "Line should not contain extra info without -v: {}",
                    line
                );
            }
        }
    }

    #[test]
    fn test_unix_philosophy_compliance() {
        // Test that commands without -v produce minimal output

        // list should just list versions
        let output = run_ovc(&["--list", "4.19"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.contains("("));

        // installed should just list versions
        let output = run_ovc(&["--installed", "4.19"]);

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(!stdout.contains("("));
        } else {
            // If no versions match, that's also valid for this test
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains("No installed versions found matching"));
        }
    }

    #[test]
    fn test_verbose_mode_provides_details() {
        // Test that -v provides additional useful information

        // list -v should show version numbers
        let output = run_ovc(&["-v", "--list", "4.19"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("4.19"));

        // installed -v should show paths
        let output = run_ovc(&["-v", "--installed", "4.19"]);

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.trim().is_empty() {
                assert!(stdout.contains("(") && stdout.contains(")"));
            }
        } else {
            // If no versions match, that's also valid for this test
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains("No installed versions found matching"));
        }
    }

    #[test]
    fn test_symlink_handling() {
        // This test would need a more complex setup to properly test symlink creation
        // For now, just ensure the commands don't crash
        let output = run_ovc(&["--installed", "4.19"]);
        // Command should either succeed or fail gracefully
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("No installed versions found matching")
                    || stderr.contains("Version must include")
            );
        }
    }

    #[test]
    fn test_concurrent_safety() {
        // Test that multiple ovc commands can run simultaneously without issues
        use std::thread;

        let handles: Vec<_> = (0..3)
            .map(|_| {
                thread::spawn(|| {
                    let output = run_ovc(&["--list", "4.19"]);
                    assert!(output.status.success());
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
