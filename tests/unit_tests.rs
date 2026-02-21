// GNU Affero General Public License v3.0 or later (see LICENSE or https://www.gnu.org/licenses/agpl.txt)
//! Comprehensive tests for the ovc library and CLI application
//!
//! This module contains both unit tests for library functions and integration tests
//! for the CLI application, ensuring 100% test coverage and validating all edge cases
//! and functionality.

use ovc::*;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Simple temporary directory that cleans up on drop
struct TestTempDir {
    path: PathBuf,
}

impl TestTempDir {
    fn new() -> std::io::Result<Self> {
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        let random_suffix = std::process::id();
        let dir_name = format!("ovc_test_{timestamp}_{random_suffix}");
        let path = std::env::temp_dir().join(dir_name);

        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TestTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

// Helper function to run ovc command and capture output
fn run_ovc(args: &[&str]) -> std::process::Output {
    Command::new("cargo")
        .args(["run", "--"])
        .args(args)
        .output()
        .expect("Failed to execute ovc command")
}

/// Build a PATH string with directories containing an `oc` binary removed.
/// Preserves cargo, rustc, and all other tools while preventing
/// "Remove the existing oc binary" errors in tests.
fn path_without_oc() -> String {
    let path = std::env::var("PATH").unwrap_or_default();
    path.split(':')
        .filter(|dir| !dir.is_empty() && !PathBuf::from(dir).join("oc").exists())
        .collect::<Vec<_>>()
        .join(":")
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
        let platforms = [Platform::LINUX_X86_64];

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
}

#[cfg(test)]
mod version_pattern_tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        assert!(matches_version_pattern("4.19.0", "4.19.0"));
        assert!(matches_version_pattern("4.1.0-rc.1", "4.1.0-rc.1"));
    }

    #[test]
    fn test_prefix_with_dot() {
        assert!(matches_version_pattern("4.19.0", "4.19"));
        assert!(matches_version_pattern("4.19.1", "4.19"));
        assert!(matches_version_pattern("4.19.10", "4.19"));
    }

    #[test]
    fn test_prefix_with_dash() {
        assert!(matches_version_pattern("4.19.0-rc.1", "4.19.0"));
        assert!(matches_version_pattern("4.19.0-alpha.1", "4.19.0"));
    }

    #[test]
    fn test_no_false_prefix_major_minor() {
        assert!(!matches_version_pattern("4.13.58", "4.1"));
        assert!(!matches_version_pattern("4.13.0", "4.1"));
    }

    #[test]
    fn test_no_false_prefix_minor_boundary() {
        assert!(!matches_version_pattern("4.10.0", "4.1"));
        assert!(!matches_version_pattern("4.190.0", "4.19"));
    }

    #[test]
    fn test_partial_minor_boundaries() {
        assert!(matches_version_pattern("4.1.0", "4.1"));
        assert!(!matches_version_pattern("4.10.0", "4.1"));
        assert!(!matches_version_pattern("4.12.0", "4.1"));
    }

    #[test]
    fn test_empty_pattern() {
        // Empty pattern: "".is_empty() means starts_with(".") and starts_with("-") are false
        assert!(!matches_version_pattern("4.19.0", ""));
    }

    #[test]
    fn test_empty_version() {
        assert!(!matches_version_pattern("", "4.19"));
    }

    #[test]
    fn test_both_empty() {
        assert!(matches_version_pattern("", ""));
    }

    #[test]
    fn test_pattern_longer_than_version() {
        assert!(!matches_version_pattern("4.19", "4.19.0"));
    }

    #[test]
    fn test_four_part_version() {
        assert!(matches_version_pattern("4.19.0.1", "4.19.0"));
        assert!(matches_version_pattern("4.19.0.1", "4.19"));
    }

    #[test]
    fn test_nested_dash_prefix() {
        assert!(matches_version_pattern("4.19.0-rc.1.2", "4.19.0-rc.1"));
        assert!(matches_version_pattern("4.19.0-rc.1.2", "4.19.0"));
    }
}

#[cfg(test)]
mod cache_unit_tests {
    use ovc::cache::{VersionCache, VersionInfo, build_version_info, format_cache_age};
    use std::collections::HashMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_version_info(version: &str, platform: &str, url: &str) -> VersionInfo {
        let mut urls = HashMap::new();
        urls.insert(platform.to_string(), url.to_string());
        VersionInfo {
            version: version.to_string(),
            urls,
        }
    }

    #[test]
    fn test_cache_new_timestamp() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let cache = VersionCache::new(vec![]);
        assert!(cache.timestamp() >= now && cache.timestamp() <= now + 2);
    }

    #[test]
    fn test_cache_new_with_versions() {
        let v1 = make_version_info("4.19.0", "linux-x86_64", "https://example.com/4.19.0");
        let v2 = make_version_info("4.20.0", "linux-x86_64", "https://example.com/4.20.0");
        let cache = VersionCache::new(vec![v1, v2]);
        assert_eq!(cache.get_version_strings().len(), 2);
    }

    #[test]
    fn test_get_version_strings_order() {
        let v1 = make_version_info("4.19.0", "linux-x86_64", "https://a");
        let v2 = make_version_info("4.20.0", "linux-x86_64", "https://b");
        let v3 = make_version_info("4.18.0", "linux-x86_64", "https://c");
        let cache = VersionCache::new(vec![v1, v2, v3]);
        let strings = cache.get_version_strings();
        assert_eq!(strings, vec!["4.19.0", "4.20.0", "4.18.0"]);
    }

    #[test]
    fn test_get_version_strings_empty() {
        let cache = VersionCache::new(vec![]);
        assert!(cache.get_version_strings().is_empty());
    }

    #[test]
    fn test_get_download_url_found() {
        let v = make_version_info("4.19.0", "linux-x86_64", "https://mirror/4.19.0.tar.gz");
        let cache = VersionCache::new(vec![v]);
        assert_eq!(
            cache.get_download_url("4.19.0", "linux-x86_64"),
            Some("https://mirror/4.19.0.tar.gz".to_string())
        );
    }

    #[test]
    fn test_get_download_url_wrong_version() {
        let v = make_version_info("4.19.0", "linux-x86_64", "https://mirror/4.19.0.tar.gz");
        let cache = VersionCache::new(vec![v]);
        assert_eq!(cache.get_download_url("4.20.0", "linux-x86_64"), None);
    }

    #[test]
    fn test_get_download_url_wrong_platform() {
        let v = make_version_info("4.19.0", "linux-x86_64", "https://mirror/4.19.0.tar.gz");
        let cache = VersionCache::new(vec![v]);
        assert_eq!(cache.get_download_url("4.19.0", "darwin-arm64"), None);
    }

    #[test]
    fn test_get_download_url_empty_cache() {
        let cache = VersionCache::new(vec![]);
        assert_eq!(cache.get_download_url("4.19.0", "linux-x86_64"), None);
    }

    #[test]
    fn test_has_version_true() {
        let v = make_version_info("4.19.0", "linux-x86_64", "https://mirror/4.19.0.tar.gz");
        let cache = VersionCache::new(vec![v]);
        assert!(cache.has_version("4.19.0"));
    }

    #[test]
    fn test_has_version_false() {
        let v = make_version_info("4.19.0", "linux-x86_64", "https://mirror/4.19.0.tar.gz");
        let cache = VersionCache::new(vec![v]);
        assert!(!cache.has_version("4.20.0"));
    }

    #[test]
    fn test_build_version_info_single() {
        let versions = vec!["4.19.0".to_string()];
        let infos = build_version_info(&versions);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].version, "4.19.0");
        let url = infos[0].urls.get("linux-x86_64").unwrap();
        assert!(url.contains("mirror.openshift.com"));
        assert!(url.contains("4.19.0"));
    }

    #[test]
    fn test_build_version_info_empty() {
        let versions: Vec<String> = vec![];
        let infos = build_version_info(&versions);
        assert!(infos.is_empty());
    }

    #[test]
    fn test_build_version_info_multiple() {
        let versions = vec![
            "4.18.0".to_string(),
            "4.19.0".to_string(),
            "4.20.0".to_string(),
        ];
        let infos = build_version_info(&versions);
        assert_eq!(infos.len(), 3);
        for (i, info) in infos.iter().enumerate() {
            assert_eq!(info.version, versions[i]);
            assert!(info.urls.contains_key("linux-x86_64"));
        }
    }

    #[test]
    fn test_format_cache_age_hours() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let result = format_cache_age(now - 7200);
        assert!(
            result.ends_with("h ago"),
            "Expected 'Nh ago', got: {result}"
        );
    }

    #[test]
    fn test_format_cache_age_minutes() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let result = format_cache_age(now - 300);
        assert!(
            result.ends_with("m ago"),
            "Expected 'Nm ago', got: {result}"
        );
    }
}

#[cfg(test)]
mod cache_integration_tests {
    use super::*;

    #[test]
    fn test_cache_roundtrip_via_list() {
        let temp_dir = TestTempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("cache");

        // First run populates the cache from network
        let output = Command::new("cargo")
            .args(["run", "--", "--list", "4.19"])
            .env("XDG_CACHE_HOME", &cache_dir)
            .env("PATH", path_without_oc())
            .output()
            .expect("Failed to execute ovc command");

        assert!(output.status.success(), "First list failed");
        assert!(
            cache_dir.join("ovc/versions.json").exists(),
            "Cache file should be created after first list"
        );

        // Second run should use cached data
        let output = Command::new("cargo")
            .args(["run", "--", "-v", "--list", "4.19"])
            .env("XDG_CACHE_HOME", &cache_dir)
            .env("PATH", path_without_oc())
            .output()
            .expect("Failed to execute ovc command");

        assert!(output.status.success(), "Second list failed");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("Using cached versions"),
            "Expected cache hit, got stderr: {stderr}"
        );
    }

    #[test]
    fn test_cache_legacy_migration() {
        let temp_dir = TestTempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("cache");
        let ovc_cache_dir = cache_dir.join("ovc");
        fs::create_dir_all(&ovc_cache_dir).unwrap();
        let cache_file = ovc_cache_dir.join("versions.json");

        // Write old-format cache
        fs::write(
            &cache_file,
            r#"{"versions":["4.19.0","4.19.0-rc.1"],"timestamp":"2024-01-01T00:00:00Z"}"#,
        )
        .unwrap();

        let output = Command::new("cargo")
            .args(["run", "--", "--list", "4.19"])
            .env("XDG_CACHE_HOME", &cache_dir)
            .env("PATH", path_without_oc())
            .output()
            .expect("Failed to execute ovc command");

        assert!(
            output.status.success(),
            "List with legacy cache failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Cache file should now be in new format with "urls" key
        let content = fs::read_to_string(&cache_file).unwrap();
        assert!(
            content.contains("urls"),
            "Cache should have been migrated to new format"
        );
    }

    #[test]
    fn test_cache_corrupted_recovery() {
        let temp_dir = TestTempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("cache");
        let ovc_cache_dir = cache_dir.join("ovc");
        fs::create_dir_all(&ovc_cache_dir).unwrap();
        let cache_file = ovc_cache_dir.join("versions.json");

        // Write corrupted cache
        fs::write(&cache_file, "{{{invalid json garbage").unwrap();

        let output = Command::new("cargo")
            .args(["run", "--", "--list", "4.19"])
            .env("XDG_CACHE_HOME", &cache_dir)
            .env("PATH", path_without_oc())
            .output()
            .expect("Failed to execute ovc command");

        assert!(
            output.status.success(),
            "List with corrupted cache failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Cache file should now have valid JSON
        let content = fs::read_to_string(&cache_file).unwrap();
        assert!(
            content.contains("versions"),
            "Cache should be valid after recovery"
        );
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
        assert!(stdout.contains("OpenShift Client Version Control"));
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
    fn test_network_error_handling() {
        let temp_dir = TestTempDir::new().unwrap();
        let output = Command::new("cargo")
            .args(["run", "--", "999.0.0"])
            .env("HOME", temp_dir.path())
            .env("XDG_CACHE_HOME", temp_dir.path().join("cache"))
            .env("PATH", path_without_oc())
            .output()
            .expect("Failed to execute ovc command");

        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("not found"),
            "Expected 'not found' error, got: {stderr}"
        );
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
        let lines: Vec<&str> = stdout.lines().collect();
        assert!(!lines.is_empty());

        for line in lines {
            if !line.trim().is_empty() {
                assert!(
                    line.starts_with("4.19"),
                    "Line should start with 4.19: {line}"
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
        let lines: Vec<&str> = stdout.lines().collect();
        assert!(!lines.is_empty());

        for line in lines {
            if !line.trim().is_empty() {
                assert!(
                    line.starts_with("4.19.0"),
                    "Line should start with 4.19.0: {line}"
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
        let lines: Vec<&str> = stdout.lines().collect();
        for line in lines {
            if !line.trim().is_empty() {
                assert!(line.starts_with("4.19"));
                // Should not contain extra verbose info like paths
                assert!(!line.contains('('));
            }
        }
    }
}

#[cfg(test)]
mod cli_installed_tests {
    use super::*;

    #[test]
    fn test_installed_command_empty() {
        // Create a temporary directory to test with clean state
        let temp_dir = TestTempDir::new().unwrap();
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
    fn test_installed_no_matches() {
        let output = run_ovc(&["--installed", "999.999"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("No installed versions found matching"));
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
    fn test_prune_verbose_mode() {
        // Test verbose mode for prune (if any versions match)
        let output = run_ovc(&["-v", "--prune", "999.999"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("No installed versions found matching"));
    }
}

#[cfg(test)]
mod cli_match_server_tests {
    use super::*;

    #[test]
    fn test_match_server_no_connection() {
        let temp_dir = TestTempDir::new().unwrap();
        let home_dir = temp_dir.path();

        let output = Command::new("cargo")
            .args(["run", "--", "--match-server"])
            .env("HOME", home_dir)
            .env("KUBECONFIG", home_dir.join("nonexistent"))
            .env("PATH", path_without_oc())
            .output()
            .expect("Failed to execute ovc command");

        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("Not connected") || stderr.contains("Failed to run"),
            "Expected cluster connection error, got: {stderr}"
        );
    }

    #[test]
    fn test_match_server_mutual_exclusivity_with_list() {
        let output = run_ovc(&["--match-server", "--list", "4.19"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("cannot be used with"));
    }

    #[test]
    fn test_match_server_mutual_exclusivity_with_prune() {
        let output = run_ovc(&["--match-server", "--prune", "4.19"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("cannot be used with"));
    }

    #[test]
    fn test_match_server_mutual_exclusivity_with_installed() {
        let output = run_ovc(&["--match-server", "--installed", "4.19"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("cannot be used with"));
    }
}

#[cfg(test)]
mod cli_update_tests {
    use super::*;

    #[test]
    fn test_update_mutual_exclusivity_with_list() {
        let output = run_ovc(&["--update", "--list", "4.19"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("cannot be used with"));
    }

    #[test]
    fn test_update_mutual_exclusivity_with_prune() {
        let output = run_ovc(&["--update", "--prune", "4.19"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("cannot be used with"));
    }

    #[test]
    fn test_update_mutual_exclusivity_with_installed() {
        let output = run_ovc(&["--update", "--installed", "4.19"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("cannot be used with"));
    }

    #[test]
    fn test_update_mutual_exclusivity_with_match_server() {
        let output = run_ovc(&["--update", "--match-server"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("cannot be used with"));
    }
}

// =============================================================================
// COMPLETION TESTS
// =============================================================================

#[cfg(test)]
mod cli_completion_tests {
    use super::*;

    #[test]
    fn test_completion_bash() {
        let output = run_ovc(&["--completion", "bash"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("_ovc_completions"));
        assert!(stdout.contains("complete -o nosort"));
    }

    #[test]
    fn test_completion_zsh_unsupported() {
        let output = run_ovc(&["--completion", "zsh"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("unsupported shell: zsh"),
            "Expected unsupported shell error, got: {stderr}"
        );
    }

    #[test]
    fn test_completion_fish_unsupported() {
        let output = run_ovc(&["--completion", "fish"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("unsupported shell: fish"),
            "Expected unsupported shell error, got: {stderr}"
        );
    }

    #[test]
    fn test_completion_bash_case_insensitive() {
        let output = run_ovc(&["--completion", "BASH"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("_ovc_completions"));
    }
}

// =============================================================================
// ISOLATED PRUNE TESTS
// =============================================================================

#[cfg(test)]
mod cli_prune_isolated_tests {
    use super::*;

    fn create_fake_binaries(home: &std::path::Path, versions: &[&str]) {
        let bin_dir = home.join(".local/bin/oc_bins/linux-x86_64");
        fs::create_dir_all(&bin_dir).unwrap();
        for v in versions {
            fs::write(bin_dir.join(format!("oc-{v}")), "fake").unwrap();
        }
    }

    #[test]
    fn test_prune_removes_matching_files() {
        let temp_dir = TestTempDir::new().unwrap();
        let home = temp_dir.path();
        create_fake_binaries(home, &["4.19.0", "4.19.1", "4.20.0"]);

        let output = Command::new("cargo")
            .args(["run", "--", "--prune", "4.19"])
            .env("HOME", home)
            .env("PATH", path_without_oc())
            .output()
            .expect("Failed to execute ovc command");

        assert!(
            output.status.success(),
            "Prune failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let bin_dir = home.join(".local/bin/oc_bins/linux-x86_64");
        assert!(
            !bin_dir.join("oc-4.19.0").exists(),
            "4.19.0 should be removed"
        );
        assert!(
            !bin_dir.join("oc-4.19.1").exists(),
            "4.19.1 should be removed"
        );
        assert!(bin_dir.join("oc-4.20.0").exists(), "4.20.0 should remain");
    }

    #[test]
    fn test_prune_verbose_shows_removal_count() {
        let temp_dir = TestTempDir::new().unwrap();
        let home = temp_dir.path();
        create_fake_binaries(home, &["4.19.0", "4.19.1"]);

        let output = Command::new("cargo")
            .args(["run", "--", "-v", "--prune", "4.19"])
            .env("HOME", home)
            .env("PATH", path_without_oc())
            .output()
            .expect("Failed to execute ovc command");

        assert!(output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("Removed 2 version(s)"),
            "Expected removal count, got: {stderr}"
        );
    }

    #[test]
    fn test_prune_invalid_format() {
        let output = run_ovc(&["--prune", "4"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Version must include at least major and minor version"));
    }

    #[test]
    fn test_prune_empty_dir_no_matches() {
        let temp_dir = TestTempDir::new().unwrap();
        let output = Command::new("cargo")
            .args(["run", "--", "--prune", "4.19"])
            .env("HOME", temp_dir.path())
            .env("PATH", path_without_oc())
            .output()
            .expect("Failed to execute ovc command");

        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("No installed versions found matching"));
    }
}

// =============================================================================
// ISOLATED INSTALLED TESTS
// =============================================================================

#[cfg(test)]
mod cli_installed_isolated_tests {
    use super::*;

    fn create_fake_binaries(home: &std::path::Path, versions: &[&str]) {
        let bin_dir = home.join(".local/bin/oc_bins/linux-x86_64");
        fs::create_dir_all(&bin_dir).unwrap();
        for v in versions {
            fs::write(bin_dir.join(format!("oc-{v}")), "fake").unwrap();
        }
    }

    #[test]
    fn test_installed_from_known_state() {
        let temp_dir = TestTempDir::new().unwrap();
        let home = temp_dir.path();
        create_fake_binaries(home, &["4.19.0", "4.19.1", "4.20.0"]);

        let output = Command::new("cargo")
            .args(["run", "--", "--installed", "4.19"])
            .env("HOME", home)
            .env("PATH", path_without_oc())
            .output()
            .expect("Failed to execute ovc command");

        assert!(
            output.status.success(),
            "Installed failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("4.19.0"));
        assert!(stdout.contains("4.19.1"));
        assert!(!stdout.contains("4.20.0"));
    }

    #[test]
    fn test_installed_verbose_shows_paths() {
        let temp_dir = TestTempDir::new().unwrap();
        let home = temp_dir.path();
        create_fake_binaries(home, &["4.19.0"]);

        let output = Command::new("cargo")
            .args(["run", "--", "-v", "--installed", "4.19"])
            .env("HOME", home)
            .env("PATH", path_without_oc())
            .output()
            .expect("Failed to execute ovc command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains('(') && stdout.contains(')'));
        assert!(stdout.contains("oc_bins"));
    }

    #[test]
    fn test_installed_no_false_prefix_match() {
        let temp_dir = TestTempDir::new().unwrap();
        let home = temp_dir.path();
        create_fake_binaries(home, &["4.1.0", "4.13.0", "4.10.0"]);

        let output = Command::new("cargo")
            .args(["run", "--", "--installed", "4.1"])
            .env("HOME", home)
            .env("PATH", path_without_oc())
            .output()
            .expect("Failed to execute ovc command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("4.1.0"));
        assert!(
            !stdout.contains("4.13.0"),
            "4.13 should not match 4.1 pattern"
        );
        assert!(
            !stdout.contains("4.10.0"),
            "4.10 should not match 4.1 pattern"
        );
    }

    #[test]
    fn test_installed_invalid_format() {
        let output = run_ovc(&["--installed", "4"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Version must include at least major and minor version"));
    }

    #[test]
    fn test_installed_sorted_output() {
        let temp_dir = TestTempDir::new().unwrap();
        let home = temp_dir.path();
        create_fake_binaries(home, &["4.19.3", "4.19.1", "4.19.10", "4.19.2"]);

        let output = Command::new("cargo")
            .args(["run", "--", "--installed", "4.19"])
            .env("HOME", home)
            .env("PATH", path_without_oc())
            .output()
            .expect("Failed to execute ovc command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        let versions: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(
            versions,
            vec!["4.19.1", "4.19.2", "4.19.3", "4.19.10"],
            "Versions should be sorted semantically"
        );
    }
}

// =============================================================================
// Man page tests
// =============================================================================

mod manpage_unit_tests {
    use super::*;
    use ovc::manpage;

    #[test]
    fn test_get_data_dir_with_xdg() {
        let temp_dir = TestTempDir::new().unwrap();
        let data_home = temp_dir.path().join("data");

        // SAFETY: test runs in a single thread; no concurrent env access
        unsafe { std::env::set_var("XDG_DATA_HOME", &data_home) };
        let dir = manpage::get_data_dir().unwrap();
        unsafe { std::env::remove_var("XDG_DATA_HOME") };

        assert_eq!(dir, data_home.join("ovc"));
        assert!(dir.exists());
    }

    #[test]
    fn test_get_data_dir_falls_back_to_home() {
        let temp_dir = TestTempDir::new().unwrap();
        let home = temp_dir.path();

        // SAFETY: test runs in a single thread; no concurrent env access
        unsafe { std::env::remove_var("XDG_DATA_HOME") };
        let saved_home = std::env::var("HOME").unwrap();
        unsafe { std::env::set_var("HOME", home) };
        let dir = manpage::get_data_dir().unwrap();
        unsafe { std::env::set_var("HOME", saved_home) };

        assert_eq!(dir, home.join(".local/share/ovc"));
        assert!(dir.exists());
    }

    #[test]
    fn test_get_man_install_dir_creates_path() {
        let temp_dir = TestTempDir::new().unwrap();
        let data_home = temp_dir.path().join("data");

        // SAFETY: test runs in a single thread; no concurrent env access
        unsafe { std::env::set_var("XDG_DATA_HOME", &data_home) };
        let dir = manpage::get_man_install_dir().unwrap();
        unsafe { std::env::remove_var("XDG_DATA_HOME") };

        assert_eq!(dir, data_home.join("man/man1"));
        assert!(dir.exists());
    }

    #[test]
    fn test_read_installed_version_missing() {
        let temp_dir = TestTempDir::new().unwrap();
        let data_home = temp_dir.path().join("data");

        // SAFETY: test runs in a single thread; no concurrent env access
        unsafe { std::env::set_var("XDG_DATA_HOME", &data_home) };
        let version = manpage::read_installed_version();
        unsafe { std::env::remove_var("XDG_DATA_HOME") };

        assert!(version.is_none());
    }

    #[test]
    fn test_write_and_read_version_file() {
        let temp_dir = TestTempDir::new().unwrap();
        let data_home = temp_dir.path().join("data");

        // SAFETY: test runs in a single thread; no concurrent env access
        unsafe { std::env::set_var("XDG_DATA_HOME", &data_home) };
        manpage::write_version_file("1.2.3").unwrap();
        let version = manpage::read_installed_version();
        unsafe { std::env::remove_var("XDG_DATA_HOME") };

        assert_eq!(version, Some("1.2.3".to_string()));
    }

    #[test]
    fn test_read_installed_version_trims_whitespace() {
        let temp_dir = TestTempDir::new().unwrap();
        let data_home = temp_dir.path().join("data");
        let ovc_dir = data_home.join("ovc");
        fs::create_dir_all(&ovc_dir).unwrap();
        fs::write(ovc_dir.join("man-version"), "1.2.3\n").unwrap();

        // SAFETY: test runs in a single thread; no concurrent env access
        unsafe { std::env::set_var("XDG_DATA_HOME", &data_home) };
        let version = manpage::read_installed_version();
        unsafe { std::env::remove_var("XDG_DATA_HOME") };

        assert_eq!(version, Some("1.2.3".to_string()));
    }
}

mod manpage_integration_tests {
    use super::*;

    #[test]
    fn test_ensure_man_page_skips_when_version_matches() {
        let temp_dir = TestTempDir::new().unwrap();
        let data_home = temp_dir.path().join("data");
        let ovc_dir = data_home.join("ovc");
        fs::create_dir_all(&ovc_dir).unwrap();

        // Write the current version so ensure_man_page skips the install
        let current_version = env!("CARGO_PKG_VERSION");
        fs::write(ovc_dir.join("man-version"), current_version).unwrap();

        let output = Command::new("cargo")
            .args(["run", "--", "--help"])
            .env("XDG_DATA_HOME", &data_home)
            .output()
            .expect("Failed to execute ovc command");

        assert!(output.status.success());
        // man1 directory should not be created when version already matches
        assert!(
            !data_home.join("man/man1/ovc.1").exists(),
            "Man page should not be written when version matches"
        );
    }

    #[test]
    fn test_ensure_man_page_installs_on_version_mismatch() {
        let temp_dir = TestTempDir::new().unwrap();
        let data_home = temp_dir.path().join("data");
        let ovc_dir = data_home.join("ovc");
        fs::create_dir_all(&ovc_dir).unwrap();

        // Write a stale version to trigger re-install
        fs::write(ovc_dir.join("man-version"), "0.0.0").unwrap();

        let output = Command::new("cargo")
            .args(["run", "--", "--help"])
            .env("XDG_DATA_HOME", &data_home)
            .output()
            .expect("Failed to execute ovc command");

        assert!(output.status.success());
        assert!(
            data_home.join("man/man1/ovc.1").exists(),
            "Man page should be installed on version mismatch"
        );
        let version = fs::read_to_string(ovc_dir.join("man-version")).unwrap();
        assert_eq!(version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn test_ensure_man_page_installs_on_first_run() {
        let temp_dir = TestTempDir::new().unwrap();
        let data_home = temp_dir.path().join("data");

        let output = Command::new("cargo")
            .args(["run", "--", "--help"])
            .env("XDG_DATA_HOME", &data_home)
            .output()
            .expect("Failed to execute ovc command");

        assert!(output.status.success());
        assert!(
            data_home.join("man/man1/ovc.1").exists(),
            "Man page should be installed on first run"
        );
        let man_content = fs::read_to_string(data_home.join("man/man1/ovc.1")).unwrap();
        assert!(
            man_content.contains("ovc"),
            "Installed man page should contain 'ovc'"
        );
    }
}
