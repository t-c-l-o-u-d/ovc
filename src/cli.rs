// SPDX-License-Identifier: AGPL-3.0-or-later
// CLI argument definitions for ovc
//
// Separated from main.rs so that build.rs can include this file
// to generate the man page via clap_mangen.

use clap::{Parser, ValueEnum};

/// Standalone actions that don't require a version argument
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StandaloneAction {
    MatchServer,
}

/// Shells with completion support
#[derive(Clone, Copy, ValueEnum)]
pub enum CompletionShell {
    Bash,
}

/// CLI argument parser - bools required for clap flag parsing
#[derive(Parser)]
// Fixed usage; generated lines pair conflicting args
#[command(
    name = "ovc",
    version,
    about = "OpenShift Client Version Control",
    disable_version_flag = true,
    override_usage = "ovc [OPTIONS] [VERSION]"
)]
#[allow(clippy::struct_excessive_bools)]
pub struct Cli {
    /// Print version
    #[arg(long = "version")]
    pub version: bool,

    /// Version to download
    #[arg(value_name = "VERSION", conflicts_with_all = ["list", "installed", "prune", "match_server"])]
    pub target_version: Option<String>,

    /// List available versions from the mirror
    #[arg(short = 'l', long = "list", value_name = "VERSION", conflicts_with_all = ["target_version", "installed", "prune", "match_server"])]
    pub list: Option<String>,

    /// List installed versions
    #[arg(short = 'i', long = "installed", value_name = "VERSION", conflicts_with_all = ["target_version", "list", "prune", "match_server"])]
    pub installed: Option<String>,

    /// Remove all installed versions
    #[arg(short = 'p', long = "prune", conflicts_with_all = ["target_version", "list", "installed", "match_server"])]
    pub prune: bool,

    /// Download the version matching the currently connected cluster
    #[arg(short = 'm', long = "match-server", conflicts_with_all = ["target_version", "list", "installed", "prune"])]
    pub match_server: bool,

    /// Allow insecure TLS connections (skip certificate verification)
    // checked in main so meta flags win
    #[arg(short = 'k', long = "insecure", conflicts_with_all = ["target_version", "list", "installed", "prune"])]
    pub insecure: bool,

    /// Make the operation more talkative
    #[arg(short, long)]
    pub verbose: bool,

    /// Generate shell completion script
    #[arg(long = "completion", value_name = "SHELL", value_enum)]
    pub completion: Option<CompletionShell>,
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
