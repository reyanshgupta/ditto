# Ditto

[![MIT license](https://img.shields.io/badge/license-MIT-6f42c1.svg)](LICENSE)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-6f42c1.svg)](https://www.rust-lang.org/)

Keep work, personal, and client Claude Code and Codex logins apart.

Ditto gives each profile its own authentication, settings, and session history. Pick a profile in the terminal, sign in through the official CLI, then launch Claude Code or Codex. Your existing setup stays available as the `default` profile.

The name is a nod to the shape-shifting Pokémon: one small tool, whichever coding identity you need.

```text
┌──────────────────── Ditto  choose a profile, then a tool ────────────────────┐
├─ Profiles ───────────────┬─ Selected profile ─────────────────────────────────┤
│  default  existing       │ work  Isolated profile                            │
│› work                    │                                                    │
│  personal                │ Authentication                                     │
│                          │ Claude Code  ● Signed in                            │
│                          │ Codex        ○ Sign in required                     │
├──────────────────────────┴────────────────────────────────────────────────────┤
│ c open Claude   x open Codex   l sign in   o sign out                         │
│ ↑/↓ select      n new profile  r refresh   q quit                             │
└───────────────────────────────────────────────────────────────────────────────┘
```

## Why use it?

Claude Code and Codex both keep user-level configuration and login state in a home directory. That works until you need separate accounts for different jobs. Manually moving auth files around is easy to get wrong, and it is hard to tell which account a new session will use.

Ditto launches each CLI with a profile-specific home directory:

| Tool | Setting used by Ditto |
| --- | --- |
| Claude Code | `CLAUDE_CONFIG_DIR=~/.ditto/profiles/<name>/claude` |
| Codex | `CODEX_HOME=~/.ditto/profiles/<name>/codex` |

No config files are swapped. Profiles remain independent, and switching only affects the process launched by Ditto.

## Requirements

Install at least one of the supported CLIs:

- [Claude Code](https://code.claude.com/docs/en/setup)
- [OpenAI Codex CLI](https://github.com/openai/codex)

Building Ditto requires Rust 1.85 or newer.

## Install

From GitHub:

```bash
cargo install --git https://github.com/reyanshgupta/ditto
```

From a local checkout:

```bash
git clone https://github.com/reyanshgupta/ditto.git
cd ditto
cargo install --path .
```

Make sure Cargo's binary directory is in your `PATH`:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

## Quick start

Open Ditto:

```bash
ditto
```

Then:

1. Press `n` and name the profile, such as `work`.
2. Select it and press `l`.
3. Choose Claude Code with `c` or Codex with `x`.
4. Complete the official login flow. Ditto returns to the profile screen afterward.
5. Press `c` or `x` to launch the tool.

Repeat the sign-in step for the other CLI if the profile uses both.

## TUI controls

| Key | Action |
| --- | --- |
| `↑` / `↓` or `k` / `j` | Select a profile |
| `c` | Launch Claude Code |
| `x` | Launch Codex |
| `l` | Sign in with Claude Code or Codex |
| `o` | Sign out, with confirmation |
| `n` | Create a profile |
| `r` | Refresh authentication status |
| `q` or `Esc` | Quit or close a dialog |

The selected profile is remembered for the next run.

## Command-line usage

The TUI is optional. Every launch command works directly from the shell:

```bash
# Profiles
ditto create work
ditto list
ditto status work
ditto paths work

# Launch a tool
ditto claude work
ditto codex work

# Pass arguments to the underlying CLI after --
ditto claude work -- --model opus
ditto codex work -- --search
```

If the profile name is omitted, Ditto uses the last selected profile. Before the first selection it uses `default`.

You can also call the native authentication commands through a profile:

```bash
ditto claude work -- auth login
ditto codex work -- login
```

## Where credentials are stored

Ditto does not ask for passwords, parse OAuth tokens, or keep credentials in its state file. The login action runs the installed Claude Code or Codex CLI with the selected profile directory, so each vendor's own authentication flow remains responsible for credential storage.

Codex keeps its auth state under the selected `CODEX_HOME`. Claude Code uses the selected `CLAUDE_CONFIG_DIR`; on macOS, sensitive Claude credentials remain in the system Keychain.

Ditto's files are laid out like this:

```text
~/.ditto/
├── state.toml
└── profiles/
    ├── work/
    │   ├── claude/
    │   └── codex/
    └── personal/
        ├── claude/
        └── codex/
```

Directories are created with user-only permissions on Unix systems.

The `default` profile points to `~/.claude` and `~/.codex`. It exposes your existing setup without copying or migrating anything.

## Environment variables

| Variable | Purpose |
| --- | --- |
| `DITTO_HOME` | Move Ditto's state and profile directory from `~/.ditto` |
| `DITTO_CLAUDE_BIN` | Override the `claude` executable |
| `DITTO_CODEX_BIN` | Override the `codex` executable |

Example:

```bash
DITTO_HOME="$HOME/.config/ditto" ditto
```

`ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, and `OPENAI_API_KEY` are inherited by launched tools. They may override a saved subscription login, so Ditto shows a warning when one is set.

## Remove Ditto

Uninstall the binary:

```bash
cargo uninstall ditto
```

Profiles are deliberately left alone. If you no longer need their settings, sessions, or credentials, remove `~/.ditto` yourself.

## Development

```bash
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## License

Ditto is available under the [MIT License](LICENSE).

Ditto is an independent project. It is not affiliated with Anthropic, OpenAI, Nintendo, or The Pokémon Company.
