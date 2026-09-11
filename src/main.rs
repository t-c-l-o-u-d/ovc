// SPDX-License-Identifier: AGPL-3.0-or-later
//! Download, switch, and prune OpenShift client binaries.

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, exit};

use clap::{CommandFactory, Parser};
use clap_complete::{Shell, generate};
use flate2::read::GzDecoder;

use tar::Archive;

mod cli;
use cli::{Cli, CompletionShell};

mod update;

use ovc::cache::{
    available_versions, fetch_and_cache_versions, load_cached_versions, refresh_missing_version,
};
use ovc::xdg::warn_cache_fallback;
use ovc::{Platform, compare_versions, find_matching_version, matches_version_pattern};

/// Parse arguments and dispatch to a command handler.
fn main() {
    // Install before parsing so --help triggers it
    ovc::manpage::ensure_man_page(false);

    let cli = Cli::parse();

    if let Some(shell) = cli.completion {
        generate(
            completion_shell(shell),
            &mut Cli::command(),
            "ovc",
            &mut io::stdout(),
        );
        return;
    }

    if cli.version {
        if cli.output.verbose {
            println!("Version: {}", env!("CARGO_PKG_VERSION"));
            println!("Source Code: https://github.com/t-c-l-o-u-d/ovc");
            println!("Author: tcloud");
        } else {
            println!("{}", env!("CARGO_PKG_VERSION"));
        }
        return;
    }

    // Checked here so meta flags win
    if cli.output.insecure && !cli.actions.match_server {
        eprintln!("ovc: --insecure requires --match-server");
        exit(1);
    }

    let verbose = cli.output.verbose;
    let insecure = cli.output.insecure;

    update::try_auto_update(verbose);

    // The action group enforces exclusivity
    let result = if let Some(version_pattern) = cli.list {
        cmd_list_available(&version_pattern, verbose)
    } else if let Some(version_pattern) = cli.installed {
        cmd_list_installed(&version_pattern, verbose)
    } else if cli.actions.prune {
        cmd_prune(verbose)
    } else if cli.actions.match_server {
        cmd_match_server(verbose, insecure)
    } else {
        match cli.target_version {
            Some(version) => cmd_download(&version, verbose),
            None => Err("ovc: missing version\nTry 'ovc --help' for more information.".into()),
        }
    };

    match result {
        Ok(Outcome::Done) => {}
        Ok(Outcome::NoMatch) => exit(1),
        Err(e) => {
            eprintln!("{e}");
            exit(1);
        }
    }
}

/// Whether a command produced any output.
enum Outcome {
    Done,
    NoMatch,
}

// =============================================================================
// Command Implementation Functions
// =============================================================================

/// Fail when an unmanaged `oc` binary sits on `PATH`.
fn reject_path_oc() -> Result<(), Box<dyn Error>> {
    if let Some(existing) = find_path_oc() {
        return Err(format!(
            "Remove the existing oc binary in ${{PATH}}: {}",
            existing.display()
        )
        .into());
    }
    Ok(())
}

/// Download a version, then set it as the default.
fn cmd_download(version: &str, verbose: bool) -> Result<Outcome, Box<dyn Error>> {
    reject_path_oc()?;
    warn_cache_fallback(verbose);

    let platform = Platform::detect();
    let resolved = resolve_version(version)?;

    if verbose && version != resolved {
        eprintln!("Resolved {version} to {resolved}");
    }

    let (path, downloaded) = ensure_oc_binary(&resolved, &platform, verbose)?;

    if verbose {
        if downloaded {
            eprintln!("Downloading: {resolved}");
            eprintln!("Downloaded to: {}", path.display());
        } else {
            eprintln!("Already installed: {resolved} ({})", path.display());
        }
    }

    set_default_oc(&resolved, &platform)?;

    if verbose {
        eprintln!("Set as default: {resolved}");
        check_path_warnings(verbose);
    }

    Ok(Outcome::Done)
}

/// Print installed versions matching a pattern.
fn cmd_list_installed(version_pattern: &str, verbose: bool) -> Result<Outcome, Box<dyn Error>> {
    require_major_minor(version_pattern)?;
    warn_cache_fallback(verbose);

    let matching: Vec<String> = list_installed_versions()?
        .into_iter()
        .filter(|v| matches_version_pattern(v, version_pattern))
        .collect();

    if matching.is_empty() {
        if verbose {
            eprintln!("No installed versions found matching {version_pattern}");
        }
        return Ok(Outcome::NoMatch);
    }

    let bin_dir = get_bin_dir()?;
    for version in matching {
        if verbose {
            println!(
                "{version} ({})",
                bin_dir.join(format!("oc-{version}")).display()
            );
        } else {
            println!("{version}");
        }
    }
    Ok(Outcome::Done)
}

/// Print mirror versions matching a pattern.
fn cmd_list_available(version_pattern: &str, verbose: bool) -> Result<Outcome, Box<dyn Error>> {
    require_major_minor(version_pattern)?;
    warn_cache_fallback(verbose);

    let matching: Vec<String> = available_versions(verbose)?
        .into_iter()
        .filter(|v| matches_version_pattern(v, version_pattern))
        .collect();

    if matching.is_empty() {
        if verbose {
            eprintln!("No versions found matching {version_pattern}");
        }
        return Ok(Outcome::NoMatch);
    }

    for version in matching {
        println!("{version}");
    }
    Ok(Outcome::Done)
}

/// Remove every installed version except the active one.
fn cmd_prune(verbose: bool) -> Result<Outcome, Box<dyn Error>> {
    warn_cache_fallback(verbose);
    let installed = list_installed_versions()?;

    if installed.is_empty() {
        return Err("No installed versions found".into());
    }

    let active = active_oc_version();
    let bin_dir = get_bin_dir()?;
    let mut removed = 0;

    for version in &installed {
        if active.as_deref() == Some(version.as_str()) {
            if verbose {
                eprintln!("Keeping active version: {version}");
            }
            continue;
        }
        let oc_path = bin_dir.join(format!("oc-{version}"));
        if oc_path.exists() {
            if verbose {
                eprintln!("Removing: {}", oc_path.display());
            }
            fs::remove_file(&oc_path)?;
            removed += 1;
        }
    }

    if verbose {
        eprintln!("Removed {removed} version(s)");
    }

    Ok(Outcome::Done)
}

/// Install the `oc` binary served by the connected cluster.
fn cmd_match_server(verbose: bool, insecure: bool) -> Result<Outcome, Box<dyn Error>> {
    reject_path_oc()?;
    warn_cache_fallback(verbose);

    let download_url = get_cluster_url(verbose)?;

    if verbose {
        eprintln!("Downloading from cluster: {download_url}");
    }

    let platform = Platform::detect();
    let bin_dir = get_bin_dir()?;
    let temp_path = bin_dir.join("oc-cluster-temp");

    download_from_cluster(&download_url, &temp_path, insecure, verbose)?;

    let version = get_binary_version(&temp_path)?;

    if verbose {
        eprintln!("Detected version: {version}");
    }

    fs::rename(&temp_path, bin_dir.join(format!("oc-{version}")))?;
    set_default_oc(&version, &platform)?;

    if verbose {
        eprintln!("Installed and set as default: {version}");
        check_path_warnings(verbose);
    }

    Ok(Outcome::Done)
}

/// Build the cluster download URL from its console URL.
fn get_cluster_url(verbose: bool) -> Result<String, Box<dyn Error>> {
    let output = Command::new("oc")
        .args(["whoami", "--show-console"])
        .output()
        .map_err(|e| format!("Cannot run 'oc': {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Not connected to a cluster. Run 'oc login' first.\n{stderr}").into());
    }

    let console_url = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if verbose {
        eprintln!("Console URL: {console_url}");
    }

    let downloads = console_url.replace("console-openshift-console", "downloads-openshift-console");
    Ok(format!("{downloads}/amd64/linux/oc.tar"))
}

/// Extract the `oc` binary from the cluster tar archive.
fn download_from_cluster(
    url: &str,
    dest: &Path,
    insecure: bool,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    let client = reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(insecure)
        .build()?;

    let resp = match client.get(url).send() {
        Ok(r) => r,
        Err(e) => {
            if verbose && e.is_connect() {
                let text = e.to_string().to_lowercase();
                if text.contains("certificate") || text.contains("ssl") || text.contains("tls") {
                    eprintln!("Use --insecure to skip certificate checks.");
                }
            }
            return Err(e.into());
        }
    };

    if !resp.status().is_success() {
        return Err(format!("Cannot download from cluster: {url} ({})", resp.status()).into());
    }

    extract_oc(Archive::new(resp), dest)
}

/// Read the client version from an `oc` binary.
fn get_binary_version(path: &Path) -> Result<String, Box<dyn Error>> {
    let output = Command::new(path)
        .arg("version")
        .arg("--client")
        .output()
        .map_err(|e| format!("Cannot run downloaded oc binary: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        if let Some(version) = line.strip_prefix("Client Version: ") {
            return Ok(version.trim().to_string());
        }
    }

    Err("Cannot read version from downloaded binary".into())
}

/// Warn when `oc` is missing or `~/.local/bin` is unreachable.
fn check_path_warnings(verbose: bool) {
    let Ok(home) = std::env::var("HOME") else {
        eprintln!("Warning: HOME environment variable not set");
        return;
    };
    let local_bin = PathBuf::from(&home).join(".local/bin");

    if !local_bin.join("oc").exists() {
        eprintln!("Warning: oc binary not found in ~/.local/bin");
        eprintln!("Run 'ovc [VERSION]' to install and set default");
        return;
    }

    let Ok(path_var) = std::env::var("PATH") else {
        eprintln!("Warning: Could not read $PATH environment variable");
        return;
    };

    let target = local_bin
        .canonicalize()
        .unwrap_or_else(|_| local_bin.clone());
    let in_path = path_var.split(':').filter(|p| !p.is_empty()).any(|p| {
        let entry = Path::new(p);
        entry
            .canonicalize()
            .map_or_else(|_| entry == local_bin, |c| c == target)
    });

    if !in_path && verbose {
        eprintln!("Warning: ~/.local/bin is not in your ${{PATH}}");
    }
}

// =============================================================================
// Version Resolution and Management Functions
// =============================================================================

/// Fail when a version lacks major and minor parts.
fn require_major_minor(version: &str) -> Result<(), Box<dyn Error>> {
    if version.split('.').count() < 2 {
        return Err("Version needs major and minor (e.g. 4.19)".into());
    }
    Ok(())
}

/// Resolve a `major.minor` version to its latest patch.
fn resolve_version(input_version: &str) -> Result<String, Box<dyn Error>> {
    require_major_minor(input_version)?;

    // A patch version needs no lookup
    if input_version.split('.').count() >= 3 {
        return Ok(input_version.to_string());
    }

    // A fetched list is already current
    let cached = load_cached_versions(false).is_some();
    let versions = available_versions(false)?;

    if let Some(latest) = find_matching_version(input_version, &versions) {
        return Ok(latest);
    }

    if cached {
        let versions = fetch_and_cache_versions(false)?;
        if let Some(latest) = find_matching_version(input_version, &versions) {
            return Ok(latest);
        }
    }

    Err(format!("No versions found matching {input_version}").into())
}

// =============================================================================
// Binary Management Functions
// =============================================================================

/// Download the version unless it is already installed.
fn ensure_oc_binary(
    version: &str,
    platform: &Platform,
    verbose: bool,
) -> Result<(PathBuf, bool), Box<dyn Error>> {
    let oc_path = get_bin_dir()?.join(format!("oc-{version}"));

    if oc_path.exists() {
        return Ok((oc_path, false));
    }

    let download_url = find_download_url(version, platform, verbose)?;

    if verbose {
        eprintln!("Downloading from: {download_url}");
    }
    download_and_extract(&oc_path, &download_url)?;
    Ok((oc_path, true))
}

/// Find the download URL in the cache, else the mirror.
fn find_download_url(
    version: &str,
    platform: &Platform,
    verbose: bool,
) -> Result<String, Box<dyn Error>> {
    let Some(cache) = load_cached_versions(verbose) else {
        // No cache, so ask the mirror
        let url = platform.build_download_url(version);
        return if url_exists(&url)? {
            Ok(url)
        } else {
            Err(version_missing(version, platform))
        };
    };

    if let Some(url) = cache.get_download_url(version, platform.name) {
        return Ok(url);
    }

    // The cache may predate the version
    if refresh_missing_version(version, verbose)?
        && let Some(url) =
            load_cached_versions(verbose).and_then(|c| c.get_download_url(version, platform.name))
    {
        return Ok(url);
    }

    Err(version_missing(version, platform))
}

/// Check whether the mirror serves a URL.
fn url_exists(url: &str) -> Result<bool, Box<dyn Error>> {
    let resp = reqwest::blocking::Client::new().head(url).send()?;
    Ok(resp.status().is_success())
}

/// Build the error for a version the mirror lacks.
fn version_missing(version: &str, platform: &Platform) -> Box<dyn Error> {
    format!("Version '{}' not found for {}", version, platform.name).into()
}

/// Get the binary directory, creating it when absent.
fn get_bin_dir() -> Result<PathBuf, Box<dyn Error>> {
    let bin_dir = ovc::xdg::cache_root()?.join("oc");
    fs::create_dir_all(&bin_dir)?;
    Ok(bin_dir)
}

/// Download a gzipped tar archive and extract `oc`.
fn download_and_extract(oc_path: &Path, download_url: &str) -> Result<(), Box<dyn Error>> {
    let resp = reqwest::blocking::get(download_url)?;
    if !resp.status().is_success() {
        return Err(format!("Cannot download: {download_url}").into());
    }
    extract_oc(Archive::new(GzDecoder::new(resp)), oc_path)
}

/// Write the archive's `oc` entry to a path, executable.
fn extract_oc<R: io::Read>(mut archive: Archive<R>, dest: &Path) -> Result<(), Box<dyn Error>> {
    for entry in archive.entries()? {
        let mut entry = entry?;
        if entry.path()?.ends_with("oc") {
            let mut out = fs::File::create(dest)?;
            io::copy(&mut entry, &mut out)?;
            set_executable(dest)?;
            return Ok(());
        }
    }

    Err("oc binary not found in archive".into())
}

/// Set executable permissions on a file.
fn set_executable(path: &Path) -> Result<(), Box<dyn Error>> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

/// List installed versions, sorted by semantic version.
fn list_installed_versions() -> Result<Vec<String>, Box<dyn Error>> {
    let bin_dir = get_bin_dir()?;
    let mut versions = vec![];

    if bin_dir.exists() {
        for entry in fs::read_dir(bin_dir)? {
            let name = entry?.file_name();
            if let Some(version) = name.to_str().and_then(|n| n.strip_prefix("oc-")) {
                versions.push(version.to_string());
            }
        }
    }

    versions.sort_by(|a, b| compare_versions(a, b));

    Ok(versions)
}

/// Read the version behind the `~/.local/bin/oc` symlink.
fn active_oc_version() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let symlink = PathBuf::from(&home).join(".local/bin/oc");
    let target = fs::read_link(&symlink).ok()?;
    let fname = target.file_name()?.to_str()?;
    fname.strip_prefix("oc-").map(String::from)
}

/// Point the `oc` and `kubectl` symlinks at a version.
fn set_default_oc(version: &str, platform: &Platform) -> Result<(), Box<dyn Error>> {
    let oc_path = get_bin_dir()?.join(format!("oc-{version}"));

    if !oc_path.exists() {
        ensure_oc_binary(version, platform, false)?;
    }

    let home = std::env::var("HOME")?;
    let local_bin = PathBuf::from(&home).join(".local/bin");
    fs::create_dir_all(&local_bin)?;

    for name in ["oc", "kubectl"] {
        let link = local_bin.join(name);
        remove_if_exists(&link)?;
        create_symlink(&oc_path, &link)?;
    }

    Ok(())
}

/// Remove a file or symlink, including a broken one.
fn remove_if_exists(path: &Path) -> Result<(), Box<dyn Error>> {
    if path.exists() || path.is_symlink() {
        fs::remove_file(path).map_err(|e| format!("Cannot remove {}: {e}", path.display()))?;
    }
    Ok(())
}

/// Create a symlink.
fn create_symlink(target: &Path, link: &Path) -> Result<(), Box<dyn Error>> {
    std::os::unix::fs::symlink(target, link).map_err(|e| {
        format!(
            "Cannot link {} to {}: {e}",
            link.display(),
            target.display()
        )
    })?;
    Ok(())
}

/// Find an `oc` binary on `PATH` outside `~/.local/bin`.
fn find_path_oc() -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    let path_var = std::env::var("PATH").ok()?;
    let home = std::env::var("HOME").ok()?;
    let local_bin = PathBuf::from(&home).join(".local/bin");

    for dir in path_var.split(':').filter(|d| !d.is_empty()) {
        // ovc manages the copy in ~/.local/bin
        if Path::new(dir) == local_bin {
            continue;
        }

        let candidate = Path::new(dir).join("oc");
        if candidate.is_file()
            && let Ok(metadata) = candidate.metadata()
            && metadata.permissions().mode() & 0o111 != 0
        {
            return Some(candidate);
        }
    }

    None
}

/// Map the CLI shell choice to a generator shell.
fn completion_shell(shell: CompletionShell) -> Shell {
    match shell {
        CompletionShell::Bash => Shell::Bash,
    }
}
