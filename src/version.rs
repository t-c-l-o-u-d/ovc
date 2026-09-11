// SPDX-License-Identifier: AGPL-3.0-or-later
//! Parse and compare OpenShift client version strings.

/// Compare two versions, ordering a pre-release below its release.
#[must_use]
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse_version = |v: &str| -> (Vec<u32>, bool, String) {
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
                // Drop trailing words such as "EUS"
                let numeric_part = part.split_whitespace().next().unwrap_or(part);
                numeric_part.parse::<u32>().ok()
            })
            .collect();

        (version_parts, is_prerelease, prerelease_suffix)
    };

    let (a_parts, a_is_prerelease, a_suffix) = parse_version(a);
    let (b_parts, b_is_prerelease, b_suffix) = parse_version(b);

    let max_len = a_parts.len().max(b_parts.len());
    for i in 0..max_len {
        let a_part = a_parts.get(i).unwrap_or(&0);
        let b_part = b_parts.get(i).unwrap_or(&0);
        match a_part.cmp(b_part) {
            std::cmp::Ordering::Equal => {}
            other => return other,
        }
    }

    match (a_is_prerelease, b_is_prerelease) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        (true, true) => a_suffix.cmp(&b_suffix),
        // Fallback for suffixes such as "EUS"
        (false, false) => a.cmp(b),
    }
}

/// Find an exact match, else the latest sharing `major.minor`.
#[must_use]
pub fn find_matching_version(
    server_version: &str,
    available_versions: &[String],
) -> Option<String> {
    if available_versions.contains(&server_version.to_string()) {
        return Some(server_version.to_string());
    }

    let server_parts: Vec<&str> = server_version.split('.').collect();
    if server_parts.len() < 2 {
        return None;
    }

    let server_major_minor = format!("{}.{}", server_parts[0], server_parts[1]);

    let mut candidates: Vec<String> = available_versions
        .iter()
        .filter(|v| matches_version_pattern(v, &server_major_minor))
        .cloned()
        .collect();

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by(|a, b| compare_versions(a, b));
    candidates.last().cloned()
}

/// Match a version prefix, requiring a dot or dash boundary.
#[must_use]
pub fn matches_version_pattern(version: &str, pattern: &str) -> bool {
    if version == pattern {
        return true;
    }

    version.starts_with(&format!("{pattern}.")) || version.starts_with(&format!("{pattern}-"))
}
