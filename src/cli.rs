use std::ffi::OsString;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "ditto-cli",
    version,
    about = "Launch Claude Code and Codex with isolated profiles"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List available profiles.
    List,
    /// Show authentication status for both tools.
    Status {
        /// Profile name. Uses the last selected profile when omitted.
        profile: Option<String>,
    },
    /// Create an isolated profile.
    Create {
        /// Profile name: letters, numbers, '-' and '_'.
        name: String,
    },
    /// Rename an isolated profile.
    Rename {
        /// Current profile name.
        profile: String,
        /// New profile name.
        new_name: String,
    },
    /// Show the Claude and Codex directories for a profile.
    Paths {
        /// Profile name. Uses the last selected profile when omitted.
        profile: Option<String>,
    },
    /// Launch Claude Code.
    Claude(LaunchArgs),
    /// Launch Codex.
    Codex(LaunchArgs),
}

#[derive(Debug, Args)]
pub struct LaunchArgs {
    /// Profile name. Uses the last selected profile when omitted.
    pub profile: Option<String>,
    /// Arguments passed to the underlying CLI. Place them after `--`.
    #[arg(last = true)]
    pub args: Vec<OsString>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_profile_rename_command() {
        let cli = Cli::try_parse_from(["ditto-cli", "rename", "work", "client"]).unwrap();

        assert!(matches!(
            cli.command,
            Some(Command::Rename {
                profile,
                new_name
            }) if profile == "work" && new_name == "client"
        ));
    }
}
