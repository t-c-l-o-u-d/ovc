// SPDX-License-Identifier: AGPL-3.0-or-later
//! Unit and integration tests for the ovc library and CLI.

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
            .map_or(0, |d| d.as_nanos());

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

// Run ovc and capture output
fn run_ovc(args: &[&str]) -> std::process::Output {
    Command::new("cargo")
        .args(["run", "--"])
        .args(args)
        .output()
        .expect("Failed to execute ovc command")
}

/// Build a PATH with any `oc`-containing directory removed.
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
    fn compare_versions_basic() {
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
    fn compare_versions_prerelease() {
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
        // Lexical order: rc.10 sorts before rc.2
        assert_eq!(
            compare_versions("4.19.0-rc.10", "4.19.0-rc.2"),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn compare_versions_patch() {
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
    fn compare_versions_complex() {
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
    fn compare_versions_eus_suffix() {
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
    fn compare_versions_edge_cases() {
        // Test edge cases
        assert_eq!(compare_versions("", ""), std::cmp::Ordering::Equal);
        assert_eq!(compare_versions("1", "1"), std::cmp::Ordering::Equal);
        assert_eq!(compare_versions("1.0", "1"), std::cmp::Ordering::Greater);
        // Invalid versions fall back to string order
        assert_eq!(
            compare_versions("invalid.version", "another.invalid"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn compare_unusual_formats() {
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
mod version_matching_tests {
    use super::*;

    #[test]
    fn matching_exact_match() {
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
    fn matching_partial_match() {
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
    fn matching_no_match() {
        let available = vec![
            "4.1.0".to_string(),
            "4.1.1".to_string(),
            "4.2.0".to_string(),
        ];

        assert_eq!(find_matching_version("4.3.0", &available), None);
        assert_eq!(find_matching_version("5.1.0", &available), None);
    }

    #[test]
    fn matching_invalid_input() {
        let available = vec!["4.1.0".to_string(), "4.2.0".to_string()];

        assert_eq!(find_matching_version("invalid", &available), None);
        assert_eq!(find_matching_version("4", &available), None);
        assert_eq!(find_matching_version("", &available), None);
    }

    #[test]
    fn matching_empty_available() {
        let available: Vec<String> = vec![];
        assert_eq!(find_matching_version("4.1.0", &available), None);
    }

    #[test]
    fn find_matching_version_prerelease() {
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
    fn find_matching_version_sorting() {
        let available = vec![
            "4.1.10".to_string(),
            "4.1.2".to_string(),
            "4.1.1".to_string(),
            "4.1.20".to_string(),
        ];

        // Latest 4.1.x patch is 4.1.20
        assert_eq!(
            find_matching_version("4.1.15", &available),
            Some("4.1.20".to_string())
        );
    }

    #[test]
    fn matching_complex_versions() {
        let available = vec![
            "4.19.0-alpha.1".to_string(),
            "4.19.0-beta.1".to_string(),
            "4.19.0-rc.1".to_string(),
            "4.19.0".to_string(),
            "4.19.1-rc.1".to_string(),
            "4.19.1".to_string(),
        ];

        // Finds latest stable in 4.19
        assert_eq!(
            find_matching_version("4.19.5", &available),
            Some("4.19.1".to_string())
        );
    }

    #[test]
    fn matching_no_false_prefix() {
        let available = vec![
            "4.1.0".to_string(),
            "4.1.5".to_string(),
            "4.10.0".to_string(),
            "4.10.3".to_string(),
            "4.13.58".to_string(),
        ];

        // 4.1.x must not match 4.10.x or 4.13.x
        assert_eq!(
            find_matching_version("4.1.2", &available),
            Some("4.1.5".to_string())
        );

        // 4.10.x must not match 4.1.x
        assert_eq!(
            find_matching_version("4.10.1", &available),
            Some("4.10.3".to_string())
        );
    }
}

#[cfg(test)]
mod platform_tests {
    use super::*;

    #[test]
    fn platform_constants() {
        // Test Linux x86_64
        assert_eq!(Platform::LINUX_X86_64.name, "linux-x86_64");
        assert_eq!(Platform::LINUX_X86_64.mirror_path, "x86_64");
        assert_eq!(Platform::LINUX_X86_64.binary_suffix, "linux");
        assert_eq!(Platform::LINUX_X86_64.file_extension, "tar.gz");
    }

    #[test]
    fn platform_detection() {
        let platform = Platform::detect();

        // Should detect a valid platform
        assert!(!platform.name.is_empty());
        assert!(!platform.mirror_path.is_empty());
        assert!(!platform.binary_suffix.is_empty());
        assert_eq!(platform.file_extension, "tar.gz");
    }

    #[test]
    fn platform_url_building() {
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
    fn all_platforms_url_building() {
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
    fn exact_match() {
        assert!(matches_version_pattern("4.19.0", "4.19.0"));
        assert!(matches_version_pattern("4.1.0-rc.1", "4.1.0-rc.1"));
    }

    #[test]
    fn prefix_with_dot() {
        assert!(matches_version_pattern("4.19.0", "4.19"));
        assert!(matches_version_pattern("4.19.1", "4.19"));
        assert!(matches_version_pattern("4.19.10", "4.19"));
    }

    #[test]
    fn prefix_with_dash() {
        assert!(matches_version_pattern("4.19.0-rc.1", "4.19.0"));
        assert!(matches_version_pattern("4.19.0-alpha.1", "4.19.0"));
    }

    #[test]
    fn false_prefix_major_minor() {
        assert!(!matches_version_pattern("4.13.58", "4.1"));
        assert!(!matches_version_pattern("4.13.0", "4.1"));
    }

    #[test]
    fn false_prefix_minor_boundary() {
        assert!(!matches_version_pattern("4.10.0", "4.1"));
        assert!(!matches_version_pattern("4.190.0", "4.19"));
    }

    #[test]
    fn partial_minor_boundaries() {
        assert!(matches_version_pattern("4.1.0", "4.1"));
        assert!(!matches_version_pattern("4.10.0", "4.1"));
        assert!(!matches_version_pattern("4.12.0", "4.1"));
    }

    #[test]
    fn empty_pattern() {
        // Empty pattern matches no prefix
        assert!(!matches_version_pattern("4.19.0", ""));
    }

    #[test]
    fn empty_version() {
        assert!(!matches_version_pattern("", "4.19"));
    }

    #[test]
    fn both_empty() {
        assert!(matches_version_pattern("", ""));
    }

    #[test]
    fn pattern_longer_than_version() {
        assert!(!matches_version_pattern("4.19", "4.19.0"));
    }

    #[test]
    fn four_part_version() {
        assert!(matches_version_pattern("4.19.0.1", "4.19.0"));
        assert!(matches_version_pattern("4.19.0.1", "4.19"));
    }

    #[test]
    fn nested_dash_prefix() {
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
    fn cache_new_timestamp() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let cache = VersionCache::new(vec![]);
        assert!(cache.timestamp() >= now && cache.timestamp() <= now + 2);
    }

    #[test]
    fn cache_new_with_versions() {
        let v1 = make_version_info("4.19.0", "linux-x86_64", "https://example.com/4.19.0");
        let v2 = make_version_info("4.20.0", "linux-x86_64", "https://example.com/4.20.0");
        let cache = VersionCache::new(vec![v1, v2]);
        assert_eq!(cache.get_version_strings().len(), 2);
    }

    #[test]
    fn get_version_strings_order() {
        let v1 = make_version_info("4.19.0", "linux-x86_64", "https://a");
        let v2 = make_version_info("4.20.0", "linux-x86_64", "https://b");
        let v3 = make_version_info("4.18.0", "linux-x86_64", "https://c");
        let cache = VersionCache::new(vec![v1, v2, v3]);
        let strings = cache.get_version_strings();
        assert_eq!(strings, vec!["4.19.0", "4.20.0", "4.18.0"]);
    }

    #[test]
    fn get_version_strings_empty() {
        let cache = VersionCache::new(vec![]);
        assert!(cache.get_version_strings().is_empty());
    }

    #[test]
    fn get_download_url_found() {
        let v = make_version_info("4.19.0", "linux-x86_64", "https://mirror/4.19.0.tar.gz");
        let cache = VersionCache::new(vec![v]);
        assert_eq!(
            cache.get_download_url("4.19.0", "linux-x86_64"),
            Some("https://mirror/4.19.0.tar.gz".to_string())
        );
    }

    #[test]
    fn download_url_wrong_version() {
        let v = make_version_info("4.19.0", "linux-x86_64", "https://mirror/4.19.0.tar.gz");
        let cache = VersionCache::new(vec![v]);
        assert_eq!(cache.get_download_url("4.20.0", "linux-x86_64"), None);
    }

    #[test]
    fn download_url_wrong_platform() {
        let v = make_version_info("4.19.0", "linux-x86_64", "https://mirror/4.19.0.tar.gz");
        let cache = VersionCache::new(vec![v]);
        assert_eq!(cache.get_download_url("4.19.0", "darwin-arm64"), None);
    }

    #[test]
    fn download_url_empty_cache() {
        let cache = VersionCache::new(vec![]);
        assert_eq!(cache.get_download_url("4.19.0", "linux-x86_64"), None);
    }

    #[test]
    fn has_version_true() {
        let v = make_version_info("4.19.0", "linux-x86_64", "https://mirror/4.19.0.tar.gz");
        let cache = VersionCache::new(vec![v]);
        assert!(cache.has_version("4.19.0"));
    }

    #[test]
    fn has_version_false() {
        let v = make_version_info("4.19.0", "linux-x86_64", "https://mirror/4.19.0.tar.gz");
        let cache = VersionCache::new(vec![v]);
        assert!(!cache.has_version("4.20.0"));
    }

    #[test]
    fn build_version_info_single() {
        let versions = vec!["4.19.0".to_string()];
        let infos = build_version_info(&versions);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].version, "4.19.0");
        let url = infos[0].urls.get("linux-x86_64").unwrap();
        assert!(url.contains("mirror.openshift.com"));
        assert!(url.contains("4.19.0"));
    }

    #[test]
    fn build_version_info_empty() {
        let versions: Vec<String> = vec![];
        let infos = build_version_info(&versions);
        assert!(infos.is_empty());
    }

    #[test]
    fn build_version_info_multiple() {
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
    fn format_cache_age_hours() {
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
    fn format_cache_age_minutes() {
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

    #[test]
    fn format_cache_age_days() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let result = format_cache_age(now - 172_800); // 48 hours
        assert_eq!(result, "2d 0h ago");
    }

    #[test]
    fn cache_is_expired_fresh() {
        let cache = VersionCache::new(vec![]);
        assert!(!cache.is_expired());
    }

    #[test]
    fn cache_is_expired_old() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let v = make_version_info("4.19.0", "linux-x86_64", "https://example.com/4.19.0");
        let cache = VersionCache::with_timestamp(vec![v], now - 73 * 3600);
        assert!(cache.is_expired());
    }

    #[test]
    fn cache_is_expired_boundary() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let v = make_version_info("4.19.0", "linux-x86_64", "https://example.com/4.19.0");
        let cache = VersionCache::with_timestamp(vec![v], now - 72 * 3600);
        assert!(cache.is_expired());
    }
}

#[cfg(test)]
mod cache_integration_tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn cache_roundtrip_via_list() {
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
    fn cache_legacy_migration() {
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

        // Migrated cache carries a urls key
        let content = fs::read_to_string(&cache_file).unwrap();
        assert!(
            content.contains("urls"),
            "Cache should have been migrated to new format"
        );
    }

    #[test]
    fn cache_expired_triggers_refresh() {
        let temp_dir = TestTempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("cache");
        let ovc_cache_dir = cache_dir.join("ovc");
        fs::create_dir_all(&ovc_cache_dir).unwrap();
        let cache_file = ovc_cache_dir.join("versions.json");

        // Timestamp 4 days old, past the TTL
        let old_timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 4 * 86400;
        // Fake version marks the stale data
        let old_cache = format!(
            r#"{{"versions":[{{"version":"99.99.99","urls":{{"linux-x86_64":"https://example.com"}}}}],"timestamp":{old_timestamp}}}"#
        );
        fs::write(&cache_file, old_cache).unwrap();

        let output = Command::new("cargo")
            .args(["run", "--", "-v", "--list", "99.99"])
            .env("XDG_CACHE_HOME", &cache_dir)
            .env("PATH", path_without_oc())
            .output()
            .expect("Failed to execute ovc command");

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stderr.contains("Cache expired"),
            "Expected cache expiration message, got: {stderr}"
        );
        assert!(
            !stdout.contains("99.99.99"),
            "Stale version should not appear in output, got: {stdout}"
        );
    }

    #[test]
    fn cache_corrupted_recovery() {
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
    fn help_command() {
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
    fn version_command() {
        let output = run_ovc(&["--version"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Prints the bare version number
        let version = env!("CARGO_PKG_VERSION");
        assert_eq!(stdout.trim(), version);
    }

    #[test]
    fn version_verbose_command() {
        let output = run_ovc(&["--version", "--verbose"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        let version = env!("CARGO_PKG_VERSION");
        assert!(stdout.contains(&format!("Version: {version}")));
        assert!(stdout.contains("Source Code: https://github.com/t-c-l-o-u-d/ovc"));
        assert!(stdout.contains("Author: tcloud"));
    }

    #[test]
    fn missing_version_error() {
        let output = run_ovc(&[]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("ovc: missing version"));
    }

    #[test]
    fn invalid_partial_version() {
        // Test that providing only major version fails
        let output = run_ovc(&["4"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Version needs major and minor"));
    }

    #[test]
    fn errors_go_to_stderr() {
        let output = run_ovc(&["invalid-version"]);
        assert!(!output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Errors belong on stderr
        assert!(stdout.trim().is_empty() || !stdout.contains("error"));
        assert!(!stderr.trim().is_empty());
    }
}

#[cfg(test)]
mod cli_download_tests {
    use super::*;

    #[test]
    fn download_invalid_version() {
        let output = run_ovc(&["999.999.999"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("not found"));
    }

    #[test]
    fn network_error_handling() {
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
    fn list_by_pattern() {
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

        // Holds rc and stable versions
        assert!(stdout.contains("4.19.0-rc"));
        assert!(stdout.contains("4.19.0"));
    }

    #[test]
    fn list_specific_patch() {
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

        // Matches 4.19.0 rc versions only
        assert!(stdout.contains("4.19.0-rc"));
        assert!(stdout.contains("4.19.0"));
        assert!(!stdout.contains("4.19.1"));
    }

    #[test]
    fn list_invalid_format() {
        let output = run_ovc(&["--list", "4"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Version needs major and minor"));
    }

    #[test]
    fn list_no_matches() {
        let output = run_ovc(&["--list", "999.999"]);
        assert!(!output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.trim().is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains("No versions found matching"));
    }

    #[test]
    fn list_no_matches_verbose() {
        let output = run_ovc(&["-v", "--list", "999.999"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("No versions found matching 999.999"));
    }

    #[test]
    fn list_available_versions_verbose() {
        let output = run_ovc(&["-v", "--list", "4.19"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Verbose adds nothing to list output
        let lines: Vec<&str> = stdout.lines().collect();
        for line in lines {
            if !line.trim().is_empty() {
                assert!(line.starts_with("4.19"));
                // No paths in list output
                assert!(!line.contains('('));
            }
        }
    }
}

#[cfg(test)]
mod cli_installed_tests {
    use super::*;

    #[test]
    fn installed_command_empty() {
        // Temporary directory gives a clean state
        let temp_dir = TestTempDir::new().unwrap();
        let home_dir = temp_dir.path();

        // Set HOME to temp directory
        let output = Command::new("cargo")
            .args(["run", "--", "--installed", "4.19"])
            .env("HOME", home_dir)
            .output()
            .expect("Failed to execute ovc command");

        // Fails quietly when none installed
        assert!(!output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.trim().is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains("No installed versions found matching"));
    }

    #[test]
    fn installed_no_matches() {
        let output = run_ovc(&["--installed", "999.999"]);
        assert!(!output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.trim().is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains("No installed versions found matching"));
    }
}

#[cfg(test)]
mod cli_prune_tests {
    use super::*;

    #[test]
    fn prune_no_versions_installed() {
        let temp_dir = TestTempDir::new().unwrap();
        let output = Command::new("cargo")
            .args(["run", "--", "--prune"])
            .env("HOME", temp_dir.path())
            .env("PATH", path_without_oc())
            .output()
            .expect("Failed to execute ovc command");

        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("No installed versions found"));
    }
}

#[cfg(test)]
mod cli_match_server_tests {
    use super::*;

    #[test]
    fn match_server_no_connection() {
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
            stderr.contains("Not connected") || stderr.contains("Cannot run"),
            "Expected cluster connection error, got: {stderr}"
        );
    }

    #[test]
    fn match_server_rejects_list() {
        let output = run_ovc(&["--match-server", "--list", "4.19"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("cannot be used with"));
    }

    #[test]
    fn match_server_rejects_prune() {
        let output = run_ovc(&["--match-server", "--prune"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("cannot be used with"));
    }

    #[test]
    fn match_server_rejects_installed() {
        let output = run_ovc(&["--match-server", "--installed", "4.19"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("cannot be used with"));
    }
}

// =============================================================================
// MUTUAL EXCLUSIVITY TESTS
// =============================================================================

#[cfg(test)]
mod cli_mutual_exclusivity_tests {
    use super::*;

    /// Every pair of actions is rejected at parse time
    fn assert_conflict(args: &[&str]) {
        let output = run_ovc(args);
        assert!(!output.status.success(), "Expected failure for {args:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("cannot be used with"),
            "Expected conflict error for {args:?}, got: {stderr}"
        );
    }

    #[test]
    fn list_conflicts_with_installed() {
        assert_conflict(&["--list", "4.19", "--installed", "4.19"]);
    }

    #[test]
    fn list_conflicts_with_prune() {
        assert_conflict(&["--list", "4.19", "--prune"]);
    }

    #[test]
    fn installed_conflicts_with_prune() {
        assert_conflict(&["--installed", "4.19", "--prune"]);
    }

    #[test]
    fn list_conflicts_target() {
        assert_conflict(&["--list", "4.19", "4.20"]);
    }

    #[test]
    fn installed_conflicts_target() {
        assert_conflict(&["--installed", "4.19", "4.20"]);
    }

    #[test]
    fn prune_conflicts_target() {
        assert_conflict(&["--prune", "4.19"]);
    }

    #[test]
    fn match_server_conflicts_target() {
        assert_conflict(&["--match-server", "4.19"]);
    }
}

// =============================================================================
// INSECURE DEPENDENCY TESTS
// =============================================================================

#[cfg(test)]
mod cli_insecure_tests {
    use super::*;

    /// --insecure is rejected without --match-server
    fn assert_rejected(args: &[&str]) {
        let output = run_ovc(args);
        assert!(!output.status.success(), "Expected failure for {args:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("--insecure") || stderr.contains("--match-server"),
            "Expected insecure error for {args:?}, got: {stderr}"
        );
    }

    #[test]
    fn insecure_alone_is_rejected() {
        let output = run_ovc(&["--insecure"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("--insecure requires --match-server"),
            "Expected dependency error, got: {stderr}"
        );
    }

    /// Meta flags outrank the --insecure dependency check
    #[test]
    fn insecure_allows_version() {
        let output = run_ovc(&["--insecure", "--version"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn insecure_allows_completion() {
        let output = run_ovc(&["--insecure", "--completion", "bash"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("complete -F _ovc"));
    }

    #[test]
    fn insecure_allows_help() {
        let output = run_ovc(&["--insecure", "--help"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("OpenShift Client Version Control"));
    }

    #[test]
    fn insecure_rejects_list() {
        assert_rejected(&["--insecure", "--list", "4.19"]);
    }

    #[test]
    fn insecure_rejects_installed() {
        assert_rejected(&["--insecure", "--installed", "4.19"]);
    }

    #[test]
    fn insecure_rejects_prune() {
        assert_rejected(&["--insecure", "--prune"]);
    }

    /// Guards a clap quirk: requires drops on conflict
    #[test]
    fn insecure_rejects_target() {
        assert_rejected(&["4.19", "--insecure"]);
    }
}

// =============================================================================
// UNRESTRICTED FLAG TESTS
// =============================================================================

#[cfg(test)]
mod cli_unrestricted_tests {
    use super::*;

    #[test]
    fn version_wins_over_list() {
        let output = run_ovc(&["--version", "--list", "4.19"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn completion_wins_over_prune() {
        let output = run_ovc(&["--completion", "bash", "--prune"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("complete -F _ovc"));
    }

    #[test]
    fn help_wins_over_installed() {
        let output = run_ovc(&["--installed", "4.19", "--help"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("OpenShift Client Version Control"));
    }

    #[test]
    fn verbose_combines_with_list() {
        let output = run_ovc(&["--list", "4.19", "--verbose"]);
        assert!(output.status.success());
    }
}

// =============================================================================
// COMPLETION TESTS
// =============================================================================

#[cfg(test)]
mod cli_completion_tests {
    use super::*;

    #[test]
    fn completion_bash() {
        let output = run_ovc(&["--completion", "bash"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("_ovc()"));
        assert!(stdout.contains("complete -F _ovc"));
    }

    /// Completions cover every flag defined in the parser
    #[test]
    fn completion_lists_flags() {
        let output = run_ovc(&["--completion", "bash"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        for flag in [
            "--version",
            "--list",
            "--installed",
            "--prune",
            "--match-server",
            "--insecure",
            "--verbose",
            "--completion",
        ] {
            assert!(stdout.contains(flag), "Missing {flag} in: {stdout}");
        }
    }

    #[test]
    fn completion_zsh_unsupported() {
        let output = run_ovc(&["--completion", "zsh"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("invalid value 'zsh'") && stderr.contains("possible values: bash"),
            "Expected invalid value error, got: {stderr}"
        );
    }

    #[test]
    fn completion_fish_unsupported() {
        let output = run_ovc(&["--completion", "fish"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("invalid value 'fish'") && stderr.contains("possible values: bash"),
            "Expected invalid value error, got: {stderr}"
        );
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

    fn set_active_version(home: &std::path::Path, version: &str) {
        let local_bin = home.join(".local/bin");
        fs::create_dir_all(&local_bin).unwrap();
        let bin_dir = home.join(".local/bin/oc_bins/linux-x86_64");
        let target = bin_dir.join(format!("oc-{version}"));
        let link = local_bin.join("oc");
        let _ = fs::remove_file(&link);
        std::os::unix::fs::symlink(&target, &link).unwrap();
    }

    #[test]
    fn prune_removes_inactive_versions() {
        let temp_dir = TestTempDir::new().unwrap();
        let home = temp_dir.path();
        create_fake_binaries(home, &["4.19.0", "4.19.1", "4.20.0"]);
        set_active_version(home, "4.20.0");

        let output = Command::new("cargo")
            .args(["run", "--", "--prune"])
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
        assert!(
            bin_dir.join("oc-4.20.0").exists(),
            "4.20.0 should remain (active)"
        );
    }

    #[test]
    fn prune_verbose_shows_count() {
        let temp_dir = TestTempDir::new().unwrap();
        let home = temp_dir.path();
        create_fake_binaries(home, &["4.19.0", "4.19.1"]);

        let output = Command::new("cargo")
            .args(["run", "--", "-v", "--prune"])
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
    fn prune_empty_dir() {
        let temp_dir = TestTempDir::new().unwrap();
        let output = Command::new("cargo")
            .args(["run", "--", "--prune"])
            .env("HOME", temp_dir.path())
            .env("PATH", path_without_oc())
            .output()
            .expect("Failed to execute ovc command");

        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("No installed versions found"));
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
    fn installed_from_known_state() {
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
    fn installed_verbose_shows_paths() {
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
    fn installed_no_matches_verbose() {
        let temp_dir = TestTempDir::new().unwrap();
        let home = temp_dir.path();
        create_fake_binaries(home, &["4.19.0"]);

        let output = Command::new("cargo")
            .args(["run", "--", "-v", "--installed", "999.999"])
            .env("HOME", home)
            .env("PATH", path_without_oc())
            .output()
            .expect("Failed to execute ovc command");

        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("No installed versions found matching 999.999"));
    }

    #[test]
    fn installed_no_false_prefix() {
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
    fn installed_invalid_format() {
        let output = run_ovc(&["--installed", "4"]);
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Version needs major and minor"));
    }

    #[test]
    fn installed_sorted_output() {
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
    fn data_dir_with_xdg() {
        let temp_dir = TestTempDir::new().unwrap();
        let data_home = temp_dir.path().join("data");

        // SAFETY: single thread, no concurrent env access
        unsafe { std::env::set_var("XDG_DATA_HOME", &data_home) };
        let dir = manpage::get_data_dir().unwrap();
        unsafe { std::env::remove_var("XDG_DATA_HOME") };

        assert_eq!(dir, data_home.join("ovc"));
        assert!(dir.exists());
    }

    #[test]
    fn data_dir_home_fallback() {
        let temp_dir = TestTempDir::new().unwrap();
        let home = temp_dir.path();

        // SAFETY: single thread, no concurrent env access
        unsafe { std::env::remove_var("XDG_DATA_HOME") };
        let saved_home = std::env::var("HOME").unwrap();
        unsafe { std::env::set_var("HOME", home) };
        let dir = manpage::get_data_dir().unwrap();
        unsafe { std::env::set_var("HOME", saved_home) };

        assert_eq!(dir, home.join(".local/share/ovc"));
        assert!(dir.exists());
    }

    #[test]
    fn man_dir_creates_path() {
        let temp_dir = TestTempDir::new().unwrap();
        let data_home = temp_dir.path().join("data");

        // SAFETY: single thread, no concurrent env access
        unsafe { std::env::set_var("XDG_DATA_HOME", &data_home) };
        let dir = manpage::get_man_install_dir().unwrap();
        unsafe { std::env::remove_var("XDG_DATA_HOME") };

        assert_eq!(dir, data_home.join("man/man1"));
        assert!(dir.exists());
    }

    #[test]
    fn read_installed_version_missing() {
        let temp_dir = TestTempDir::new().unwrap();
        let data_home = temp_dir.path().join("data");

        // SAFETY: single thread, no concurrent env access
        unsafe { std::env::set_var("XDG_DATA_HOME", &data_home) };
        let version = manpage::read_installed_version();
        unsafe { std::env::remove_var("XDG_DATA_HOME") };

        assert!(version.is_none());
    }

    #[test]
    fn write_and_read_version() {
        let temp_dir = TestTempDir::new().unwrap();
        let data_home = temp_dir.path().join("data");

        // SAFETY: single thread, no concurrent env access
        unsafe { std::env::set_var("XDG_DATA_HOME", &data_home) };
        manpage::write_version_file("1.2.3").unwrap();
        let version = manpage::read_installed_version();
        unsafe { std::env::remove_var("XDG_DATA_HOME") };

        assert_eq!(version, Some("1.2.3".to_string()));
    }

    #[test]
    fn read_version_trims_whitespace() {
        let temp_dir = TestTempDir::new().unwrap();
        let data_home = temp_dir.path().join("data");
        let ovc_dir = data_home.join("ovc");
        fs::create_dir_all(&ovc_dir).unwrap();
        fs::write(ovc_dir.join("man-version"), "1.2.3\n").unwrap();

        // SAFETY: single thread, no concurrent env access
        unsafe { std::env::set_var("XDG_DATA_HOME", &data_home) };
        let version = manpage::read_installed_version();
        unsafe { std::env::remove_var("XDG_DATA_HOME") };

        assert_eq!(version, Some("1.2.3".to_string()));
    }
}

mod manpage_integration_tests {
    use super::*;

    #[test]
    fn skips_when_matching() {
        let temp_dir = TestTempDir::new().unwrap();
        let data_home = temp_dir.path().join("data");
        let ovc_dir = data_home.join("ovc");
        fs::create_dir_all(&ovc_dir).unwrap();

        // Current version makes ensure_man_page skip
        let current_version = env!("CARGO_PKG_VERSION");
        fs::write(ovc_dir.join("man-version"), current_version).unwrap();

        let output = Command::new("cargo")
            .args(["run", "--", "--help"])
            .env("XDG_DATA_HOME", &data_home)
            .output()
            .expect("Failed to execute ovc command");

        assert!(output.status.success());
        // man1 stays absent when version matches
        assert!(
            !data_home.join("man/man1/ovc.1").exists(),
            "Man page should not be written when version matches"
        );
    }

    #[test]
    fn installs_on_mismatch() {
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
    fn installs_on_first_run() {
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
