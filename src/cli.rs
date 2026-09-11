// SPDX-License-Identifier: AGPL-3.0-or-later
// Included by build.rs for the man page

use clap::{Args, Parser, ValueEnum};

/// Standalone actions that need no version argument.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StandaloneAction {
    MatchServer,
}

/// Shells with completion support.
#[derive(Clone, Copy, ValueEnum)]
pub enum CompletionShell {
    Bash,
}

/// Actions that run without a version argument.
#[derive(Args)]
pub struct ActionFlags {
    /// Remove inactive versions
    #[arg(short = 'p', long = "prune", conflicts_with_all = ["target_version", "list", "installed", "match_server"])]
    pub prune: bool,

    /// Download the version matching the currently connected cluster
    #[arg(short = 'm', long = "match-server", conflicts_with_all = ["target_version", "list", "installed", "prune"])]
    pub match_server: bool,
}

/// Output and connection options.
#[derive(Args)]
pub struct OutputFlags {
    /// Allow insecure TLS connections (skip certificate verification)
    // checked in main so meta flags win
    #[arg(short = 'k', long = "insecure", conflicts_with_all = ["target_version", "list", "installed", "prune"])]
    pub insecure: bool,

    /// Show detailed output
    #[arg(short, long)]
    pub verbose: bool,
}

/// Command line arguments for `ovc`.
#[derive(Parser)]
// Fixed usage; generated lines pair conflicting args
#[command(
    name = "ovc",
    version,
    about = "OpenShift Client Version Control",
    disable_version_flag = true,
    override_usage = "ovc [OPTIONS] [VERSION]"
)]
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

    #[command(flatten)]
    pub actions: ActionFlags,

    #[command(flatten)]
    pub output: OutputFlags,

    /// Generate shell completion script
    #[arg(long = "completion", value_name = "SHELL", value_enum)]
    pub completion: Option<CompletionShell>,
}

impl Cli {
    /// Return the requested standalone action, if any.
    #[must_use]
    pub fn standalone_action(&self) -> Option<StandaloneAction> {
        if self.actions.match_server {
            Some(StandaloneAction::MatchServer)
        } else {
            None
        }
    }
}
