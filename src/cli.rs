// GNU Affero General Public License v3.0 or later (see LICENSE or https://www.gnu.org/licenses/agpl.txt)
// CLI argument definitions for ovc
//
// Separated from main.rs so that build.rs can include this file
// to generate the man page via clap_mangen.

use clap::Parser;

/// Standalone actions that don't require a version argument
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StandaloneAction {
    MatchServer,
    Update,
}

/// CLI argument parser - bools required for clap flag parsing
#[derive(Parser)]
#[command(
    name = "ovc",
    version,
    about = "OpenShift Client Version Control",
    disable_version_flag = true
)]
#[command(arg(clap::Arg::new("version").long("version").action(clap::ArgAction::Version).help("Print version")))]
#[allow(clippy::struct_excessive_bools)]
pub struct Cli {
    /// Version to download
    #[arg(value_name = "VERSION")]
    pub target_version: Option<String>,

    /// List available versions from the mirror
    #[arg(short = 'l', long = "list", value_name = "VERSION")]
    pub list: Option<String>,

    /// List installed versions
    #[arg(short = 'i', long = "installed", value_name = "VERSION")]
    pub installed: Option<String>,

    /// Remove installed versions
    #[arg(short = 'p', long = "prune", value_name = "VERSION")]
    pub prune: Option<String>,

    /// Download the version matching the currently connected cluster
    #[arg(short = 'm', long = "match-server", conflicts_with_all = ["update", "list", "installed", "prune"])]
    pub match_server: bool,

    /// Update ovc to the latest version from GitHub releases
    #[arg(short = 'u', long = "update", conflicts_with_all = ["match_server", "list", "installed", "prune"])]
    pub update: bool,

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
        match (self.match_server, self.update) {
            (true, _) => Some(StandaloneAction::MatchServer),
            (_, true) => Some(StandaloneAction::Update),
            _ => None,
        }
    }
}

fn parse_completion_shell(s: &str) -> Result<String, String> {
    match s.to_lowercase().as_str() {
        "bash" => Ok(s.to_lowercase()),
        _ => Err(format!("unsupported shell: {s} (only 'bash' is supported)")),
    }
}
