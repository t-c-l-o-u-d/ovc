// SPDX-License-Identifier: AGPL-3.0-or-later
// CLI argument definitions for ovc
//
// Separated from main.rs so that build.rs can include this file
// to generate the man page via clap_mangen.

use clap::Parser;

/// Standalone actions that don't require a version argument
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StandaloneAction {
    MatchServer,
}

/// CLI argument parser - bools required for clap flag parsing
#[derive(Parser)]
#[command(
    name = "ovc",
    version,
    about = "OpenShift Client Version Control",
    disable_version_flag = true
)]
#[allow(clippy::struct_excessive_bools)]
pub struct Cli {
    /// Print version
    #[arg(long = "version")]
    pub version: bool,

    /// Version to download
    #[arg(value_name = "VERSION")]
    pub target_version: Option<String>,

    /// List available versions from the mirror
    #[arg(short = 'l', long = "list", value_name = "VERSION")]
    pub list: Option<String>,

    /// List installed versions
    #[arg(short = 'i', long = "installed", value_name = "VERSION")]
    pub installed: Option<String>,

    /// Remove all installed versions
    #[arg(short = 'p', long = "prune", conflicts_with_all = ["list", "installed", "match_server"])]
    pub prune: bool,

    /// Download the version matching the currently connected cluster
    #[arg(short = 'm', long = "match-server", conflicts_with_all = ["list", "installed", "prune"])]
    pub match_server: bool,

    /// Allow insecure TLS connections (skip certificate verification)
    #[arg(short = 'k', long = "insecure")]
    pub insecure: bool,

    /// Make the operation more talkative
    #[arg(short, long)]
    pub verbose: bool,

    /// Generate shell completion script (only bash is supported currently)
    #[arg(long = "completion", value_name = "SHELL", value_parser = parse_completion_shell)]
    pub completion: Option<String>,
}

impl Cli {
    #[must_use]
    pub fn standalone_action(&self) -> Option<StandaloneAction> {
        if self.match_server {
            Some(StandaloneAction::MatchServer)
        } else {
            None
        }
    }
}

fn parse_completion_shell(s: &str) -> Result<String, String> {
    match s.to_lowercase().as_str() {
        "bash" => Ok(s.to_lowercase()),
        _ => Err(format!("unsupported shell: {s} (only 'bash' is supported)")),
    }
}
