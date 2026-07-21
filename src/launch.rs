use std::{
    ffi::OsString,
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::profile::Profile;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tool {
    Claude,
    Codex,
}

impl Tool {
    pub fn label(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
        }
    }

    fn executable(self) -> OsString {
        let override_variable = match self {
            Self::Claude => "DITTO_CLAUDE_BIN",
            Self::Codex => "DITTO_CODEX_BIN",
        };
        std::env::var_os(override_variable).unwrap_or_else(|| match self {
            Self::Claude => OsString::from("claude"),
            Self::Codex => OsString::from("codex"),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthOperation {
    Login,
    Logout,
}

impl AuthOperation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Login => "Sign in",
            Self::Logout => "Sign out",
        }
    }

    fn args(self, tool: Tool) -> &'static [&'static str] {
        match (self, tool) {
            (Self::Login, Tool::Claude) => &["auth", "login"],
            (Self::Login, Tool::Codex) => &["login"],
            (Self::Logout, Tool::Claude) => &["auth", "logout"],
            (Self::Logout, Tool::Codex) => &["logout"],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthStatus {
    SignedIn,
    SignedOut,
    Unavailable,
}

#[derive(Deserialize)]
struct ClaudeAuthStatus {
    #[serde(rename = "loggedIn")]
    logged_in: bool,
}

pub fn build_command(tool: Tool, profile: &Profile, args: &[OsString]) -> Command {
    let mut command = base_command(tool, profile);
    command.args(args);
    command
}

pub fn auth_status(tool: Tool, profile: &Profile) -> AuthStatus {
    let mut command = base_command(tool, profile);
    match tool {
        Tool::Claude => {
            command.args(["auth", "status", "--json"]);
        }
        Tool::Codex => {
            command.args(["login", "status"]);
        }
    }

    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    let Ok(output) = output else {
        return AuthStatus::Unavailable;
    };

    match tool {
        Tool::Claude => parse_claude_auth_status(&output.stdout),
        Tool::Codex => {
            parse_codex_auth_status(output.status.success(), &output.stdout, &output.stderr)
        }
    }
}

fn parse_claude_auth_status(stdout: &[u8]) -> AuthStatus {
    serde_json::from_slice::<ClaudeAuthStatus>(stdout)
        .map(|status| {
            if status.logged_in {
                AuthStatus::SignedIn
            } else {
                AuthStatus::SignedOut
            }
        })
        .unwrap_or(AuthStatus::Unavailable)
}

fn parse_codex_auth_status(success: bool, stdout: &[u8], stderr: &[u8]) -> AuthStatus {
    if success {
        return AuthStatus::SignedIn;
    }

    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    if stdout.trim() == "Not logged in" || stderr.trim() == "Not logged in" {
        AuthStatus::SignedOut
    } else {
        AuthStatus::Unavailable
    }
}

pub fn authenticate(operation: AuthOperation, tool: Tool, profile: &Profile) -> Result<()> {
    let status = base_command(tool, profile)
        .args(operation.args(tool))
        .status()
        .with_context(|| format!("could not run {} authentication", tool.label()))?;

    if !status.success() {
        bail!(
            "{} for {} failed with {status}",
            operation.label(),
            tool.label()
        );
    }
    Ok(())
}

fn base_command(tool: Tool, profile: &Profile) -> Command {
    let mut command = Command::new(tool.executable());
    match tool {
        Tool::Claude => {
            command.env("CLAUDE_CONFIG_DIR", &profile.claude_home);
        }
        Tool::Codex => {
            command.env("CODEX_HOME", &profile.codex_home);
        }
    }
    command
}

#[cfg(unix)]
pub fn launch(tool: Tool, profile: &Profile, args: &[OsString]) -> Result<()> {
    use std::os::unix::process::CommandExt;

    let error = build_command(tool, profile, args).exec();
    Err(error).with_context(|| format!("could not launch {}", tool.label()))
}

#[cfg(not(unix))]
pub fn launch(tool: Tool, profile: &Profile, args: &[OsString]) -> Result<()> {
    let status = build_command(tool, profile, args)
        .status()
        .with_context(|| format!("could not launch {}", tool.label()))?;

    if status.success() {
        Ok(())
    } else {
        bail!("{} exited with {status}", tool.label())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn profile() -> Profile {
        Profile {
            name: "work".to_owned(),
            claude_home: PathBuf::from("/profiles/work/claude"),
            codex_home: PathBuf::from("/profiles/work/codex"),
            managed: true,
        }
    }

    #[test]
    fn claude_uses_the_selected_config_directory() {
        let command = build_command(Tool::Claude, &profile(), &[]);
        let configured_home = command
            .get_envs()
            .find(|(name, _)| *name == "CLAUDE_CONFIG_DIR")
            .and_then(|(_, value)| value);

        assert_eq!(
            configured_home,
            Some(std::ffi::OsStr::new("/profiles/work/claude"))
        );
    }

    #[test]
    fn codex_uses_the_selected_home() {
        let command = build_command(Tool::Codex, &profile(), &[]);
        let configured_home = command
            .get_envs()
            .find(|(name, _)| *name == "CODEX_HOME")
            .and_then(|(_, value)| value);

        assert_eq!(
            configured_home,
            Some(std::ffi::OsStr::new("/profiles/work/codex"))
        );
    }

    #[test]
    fn authentication_uses_native_cli_commands() {
        assert_eq!(AuthOperation::Login.args(Tool::Claude), ["auth", "login"]);
        assert_eq!(AuthOperation::Login.args(Tool::Codex), ["login"]);
        assert_eq!(AuthOperation::Logout.args(Tool::Claude), ["auth", "logout"]);
        assert_eq!(AuthOperation::Logout.args(Tool::Codex), ["logout"]);
    }

    #[test]
    fn parses_native_auth_status_output() {
        assert_eq!(
            parse_claude_auth_status(br#"{"loggedIn":true}"#),
            AuthStatus::SignedIn
        );
        assert_eq!(
            parse_claude_auth_status(br#"{"loggedIn":false}"#),
            AuthStatus::SignedOut
        );
        assert_eq!(
            parse_codex_auth_status(false, b"", b"Not logged in\n"),
            AuthStatus::SignedOut
        );
        assert_eq!(
            parse_codex_auth_status(true, b"Logged in using ChatGPT\n", b""),
            AuthStatus::SignedIn
        );
        assert_eq!(
            parse_codex_auth_status(false, b"", b"configuration error\n"),
            AuthStatus::Unavailable
        );
    }
}
