mod cli;
mod launch;
mod profile;
mod ui;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command, LaunchArgs};
use launch::{AuthStatus, Tool};
use profile::{DEFAULT_PROFILE, Profile, Store};

fn main() {
    if let Err(error) = run() {
        eprintln!("ditto-cli: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let store = Store::discover()?;

    match cli.command {
        None => run_tui(&store),
        Some(Command::List) => list_profiles(&store),
        Some(Command::Status { profile }) => show_status(&store, profile.as_deref()),
        Some(Command::Create { name }) => create_profile(&store, &name),
        Some(Command::Rename { profile, new_name }) => rename_profile(&store, &profile, &new_name),
        Some(Command::Paths { profile }) => show_paths(&store, profile.as_deref()),
        Some(Command::Claude(arguments)) => launch_direct(&store, Tool::Claude, arguments),
        Some(Command::Codex(arguments)) => launch_direct(&store, Tool::Codex, arguments),
    }
}

fn run_tui(store: &Store) -> Result<()> {
    loop {
        let profiles = store.list_profiles()?;
        let last_profile = store.last_profile()?;
        let Some(action) = ui::run(store, profiles, last_profile.as_deref())? else {
            return Ok(());
        };

        match action {
            ui::UiAction::Launch { tool, profile } => {
                store.save_last_profile(&profile.name)?;
                return launch::launch(tool, &profile, &[]);
            }
            ui::UiAction::Authenticate {
                operation,
                tool,
                profile,
            } => {
                store.save_last_profile(&profile.name)?;
                launch::authenticate(operation, tool, &profile)?;
            }
        }
    }
}

fn list_profiles(store: &Store) -> Result<()> {
    let last_profile = store.last_profile()?;
    for profile in store.list_profiles()? {
        let selected = if last_profile.as_deref() == Some(&profile.name) {
            "*"
        } else {
            " "
        };
        let kind = if profile.managed {
            "isolated"
        } else {
            "native"
        };
        println!("{selected} {:<32} {kind}", profile.name);
    }
    Ok(())
}
fn show_status(store: &Store, requested_profile: Option<&str>) -> Result<()> {
    let profile = resolve_profile(store, requested_profile)?;
    println!("{}", profile.name);
    print_auth_status(Tool::Claude, launch::auth_status(Tool::Claude, &profile));
    print_auth_status(Tool::Codex, launch::auth_status(Tool::Codex, &profile));
    Ok(())
}

fn print_auth_status(tool: Tool, status: AuthStatus) {
    let status = match status {
        AuthStatus::SignedIn => "signed in",
        AuthStatus::SignedOut => "sign in required",
        AuthStatus::Unavailable => "CLI or status unavailable",
    };
    println!("  {:<13} {status}", tool.label());
}

fn create_profile(store: &Store, name: &str) -> Result<()> {
    let profile = store.create_profile(name)?;
    println!("Created profile '{}'.", profile.name);
    print_login_instructions(&profile);
    Ok(())
}
fn rename_profile(store: &Store, current_name: &str, new_name: &str) -> Result<()> {
    let profile = store.rename_profile(current_name, new_name)?;
    println!("Renamed profile '{current_name}' to '{}'.", profile.name);
    Ok(())
}

fn show_paths(store: &Store, requested_profile: Option<&str>) -> Result<()> {
    let profile = resolve_profile(store, requested_profile)?;
    println!("profile={}", profile.name);
    println!("claude={}", profile.claude_home.display());
    println!("codex={}", profile.codex_home.display());
    Ok(())
}

fn launch_direct(store: &Store, tool: Tool, arguments: LaunchArgs) -> Result<()> {
    let profile = resolve_profile(store, arguments.profile.as_deref())?;
    store.save_last_profile(&profile.name)?;
    launch::launch(tool, &profile, &arguments.args)
}

fn resolve_profile(store: &Store, requested_profile: Option<&str>) -> Result<Profile> {
    let name = match requested_profile {
        Some(name) => name.to_owned(),
        None => store
            .last_profile()?
            .unwrap_or_else(|| DEFAULT_PROFILE.to_owned()),
    };
    store.load_profile(&name)
}

fn print_login_instructions(profile: &Profile) {
    println!();
    println!(
        "Open `ditto-cli`, select '{}', then press l to sign in.",
        profile.name
    );
    println!();
    println!("Or authenticate directly:");
    println!("  ditto-cli claude {} -- auth login", profile.name);
    println!("  ditto-cli codex {} -- login", profile.name);
}
