// SPDX-License-Identifier: AGPL-3.0-or-later

use std::error::Error;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::blocking::Client;
use sha2::{Digest, Sha256};

use ovc::compare_versions;

const GITHUB_OWNER: &str = "t-c-l-o-u-d";
const GITHUB_REPO: &str = "ovc";
const UPDATE_COOLDOWN: Duration = Duration::from_hours(24);

/// Update ovc to the latest GitHub release. Non-fatal.
pub fn try_auto_update(verbose: bool) {
    if let Err(e) = run_update(verbose)
        && verbose
    {
        eprintln!("ovc: auto-update failed: {e}");
    }
}

/// Download, verify, and install a newer release.
fn run_update(verbose: bool) -> Result<(), Box<dyn Error>> {
    if !cooldown_elapsed() {
        return Ok(());
    }
    // Update proceeds even if stamping fails
    if let Err(e) = record_cooldown()
        && verbose
    {
        eprintln!("ovc: cannot record update cooldown: {e}");
    }

    let current = env!("CARGO_PKG_VERSION");
    if verbose {
        eprintln!("ovc: checking for updates (current: v{current})");
    }

    let client = http_client()?;
    let (latest, bin_url, sha_url) = get_latest_github_release(&client, verbose)?;

    if compare_versions(&latest, current) != std::cmp::Ordering::Greater {
        if verbose {
            eprintln!("ovc: already up to date (v{current})");
        }
        return Ok(());
    }

    if verbose {
        eprintln!("ovc: downloading v{latest} from {bin_url}");
    }

    let exe = std::env::current_exe()?;
    let tmp = exe.with_extension("update");
    download_file(&client, &bin_url, &tmp)?;
    verify_sha256(&client, &sha_url, &tmp, verbose)?;
    replace_binary(&tmp, &exe, verbose)?;

    if verbose {
        eprintln!("ovc: updated from v{current} to v{latest}");
    }
    Ok(())
}

/// Build an HTTP client identifying this ovc version.
fn http_client() -> Result<Client, reqwest::Error> {
    Client::builder()
        .user_agent(format!("ovc/{}", env!("CARGO_PKG_VERSION")))
        .build()
}

/// Get the latest release version, binary URL, and checksum URL.
fn get_latest_github_release(
    client: &Client,
    verbose: bool,
) -> Result<(String, String, String), Box<dyn Error>> {
    let api_url =
        format!("https://api.github.com/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases/latest");

    if verbose {
        eprintln!("ovc: fetching release info from {api_url}");
    }

    let resp = client.get(&api_url).send()?;

    if !resp.status().is_success() {
        return Err(format!("Cannot fetch release info: {} ({})", api_url, resp.status()).into());
    }

    let release: serde_json::Value = serde_json::from_str(&resp.text()?)?;

    let tag_name = release["tag_name"]
        .as_str()
        .ok_or("No tag_name in release")?;
    let version = tag_name.strip_prefix('v').unwrap_or(tag_name);

    let assets = release["assets"].as_array().ok_or("No assets in release")?;

    let mut bin_url: Option<String> = None;
    let mut sha_url: Option<String> = None;
    for asset in assets {
        let Some(name) = asset["name"].as_str() else {
            continue;
        };
        let Some(url) = asset["browser_download_url"].as_str() else {
            continue;
        };
        if name.contains("linux") && (name.contains("x86_64") || name.contains("amd64")) {
            if name.ends_with(".sha256") {
                sha_url = Some(url.to_string());
            } else {
                bin_url = Some(url.to_string());
            }
        }
    }

    let bin_url = bin_url.ok_or("No linux-x86_64 binary found in release assets")?;
    let sha_url = sha_url.ok_or("No linux-x86_64 .sha256 found in release assets")?;

    Ok((version.to_string(), bin_url, sha_url))
}

/// Download a URL to a path and mark it executable.
fn download_file(client: &Client, url: &str, dest: &Path) -> Result<(), Box<dyn Error>> {
    use std::os::unix::fs::PermissionsExt;

    let resp = client.get(url).send()?;

    if !resp.status().is_success() {
        return Err(format!("Cannot download update: {} ({})", url, resp.status()).into());
    }

    let bytes = resp.bytes()?;
    fs::write(dest, &bytes)?;
    fs::set_permissions(dest, fs::Permissions::from_mode(0o755))?;

    Ok(())
}

/// Check the download against its published checksum.
fn verify_sha256(
    client: &Client,
    sha_url: &str,
    bin_path: &Path,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    let resp = client.get(sha_url).send()?;

    if !resp.status().is_success() {
        return Err(format!("Cannot download checksum: {} ({})", sha_url, resp.status()).into());
    }

    let expected = resp
        .text()?
        .split_whitespace()
        .next()
        .ok_or("malformed sha256 file")?
        .to_lowercase();

    let bytes = fs::read(bin_path)?;
    let actual = Sha256::digest(&bytes)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            write!(s, "{b:02x}").expect("write to String is infallible");
            s
        });

    if actual != expected {
        if let Err(e) = fs::remove_file(bin_path)
            && verbose
        {
            eprintln!("ovc: cannot remove bad download: {e}");
        }
        return Err(format!("sha256 mismatch (expected {expected}, got {actual})").into());
    }

    Ok(())
}

/// Swap in the new binary, restoring the old one on failure.
fn replace_binary(
    new_binary: &Path,
    current_binary: &Path,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    let old_path = current_binary.with_extension("old");

    // A missing backup is the normal case
    if let Err(e) = fs::remove_file(&old_path)
        && e.kind() != io::ErrorKind::NotFound
    {
        return Err(format!("Cannot clear stale backup: {e}").into());
    }

    fs::rename(current_binary, &old_path)
        .map_err(|e| format!("Cannot back up current binary: {e}"))?;

    if let Err(e) = fs::rename(new_binary, current_binary) {
        if let Err(restore) = fs::rename(&old_path, current_binary) {
            return Err(format!(
                "Install failed ({e}); restore {} manually: {restore}",
                old_path.display()
            )
            .into());
        }
        return Err(format!("Cannot install new binary: {e}").into());
    }

    if let Err(e) = fs::remove_file(&old_path)
        && verbose
    {
        eprintln!("ovc: cannot remove backup {}: {e}", old_path.display());
    }
    Ok(())
}

/// Check whether the update cooldown has passed.
pub fn cooldown_elapsed() -> bool {
    let Some(path) = cooldown_path() else {
        return true;
    };
    let Ok(meta) = fs::metadata(&path) else {
        return true;
    };
    let Ok(modified) = meta.modified() else {
        return true;
    };
    let Ok(age) = std::time::SystemTime::now().duration_since(modified) else {
        return true;
    };
    age >= UPDATE_COOLDOWN
}

/// Stamp the cooldown file with this check.
pub fn record_cooldown() -> io::Result<()> {
    let Some(path) = cooldown_path() else {
        return Ok(());
    };
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&path, "")
}

pub fn cooldown_path() -> Option<PathBuf> {
    Some(ovc::xdg::cache_root().ok()?.join("last-update-check"))
}
