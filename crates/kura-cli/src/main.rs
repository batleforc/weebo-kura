//! `kura`: the reference client (RFC 0001 §4.5, §6.8). It links `kura-core`
//! natively; exit codes are those of §4.5.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use kura_cli::commands::{self, Ctx, DeviceAction, Fail, TokenAction};

#[derive(Parser)]
#[command(
    name = "kura",
    version,
    about = "Sync Obsidian vaults with a kura server"
)]
struct Cli {
    /// A file holding a scoped token (`kura_…`), for `attach` and `sync`
    /// without `kura login`; preferred to the `KURA_TOKEN` variable, which
    /// other processes of the user can read. The server comes from
    /// `KURA_SERVER`.
    #[arg(long, global = true)]
    token_file: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Sign in with the device code flow and register this device.
    Login {
        #[arg(long)]
        server: String,
        /// The OIDC issuer, e.g. https://auth.example.org/application/o/kura/
        #[arg(long)]
        issuer: String,
        #[arg(long, default_value = "kura-cli")]
        client_id: String,
        /// Shown in device lists and conflict file names; the host name by default.
        #[arg(long)]
        device_name: Option<String>,
    },
    /// Attach a local folder to a vault, showing what will move first.
    Attach {
        vault: String,
        folder: PathBuf,
        /// Do not ask before the first sync.
        #[arg(long)]
        yes: bool,
    },
    /// Push local changes and pull remote ones, for one vault or all.
    Sync {
        vault: Option<String>,
        /// Only list what would be pushed and what cannot sync.
        #[arg(long)]
        dry_run: bool,
        /// Let a full resync move many files to .trash without asking.
        #[arg(long)]
        yes: bool,
    },
    /// Keep a vault in sync: pull on every notification, push local edits.
    Watch { vault: Option<String> },
    /// The state of every attached vault.
    Status,
    /// Check that the server holds exactly this folder's notes (migration, §13).
    Verify { vault: String },
    /// The revisions of a file, newest first.
    History { vault: String, path: String },
    /// Bring back a revision of a file as its new version.
    Restore {
        vault: String,
        path: String,
        revision: uuid::Uuid,
    },
    /// List devices, or revoke one by name.
    Devices {
        #[command(subcommand)]
        action: Option<DeviceAction>,
    },
    /// Search a vault's notes and file names.
    Search { vault: String, query: String },
    /// Scoped tokens for agents and scripts: folders they read or write.
    Token {
        #[command(subcommand)]
        action: TokenAction,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let home = match kura_cli::config::default_home() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    // A scoped token (RFC 0004 §6.6): the file, else `KURA_TOKEN`. It is
    // never written to the configuration or printed.
    let scoped = match &cli.token_file {
        Some(p) => match std::fs::read_to_string(p) {
            Ok(t) => Some(t.trim().to_owned()),
            Err(e) => {
                eprintln!("error: {}: {e}", p.display());
                return ExitCode::from(1);
            }
        },
        None => std::env::var("KURA_TOKEN").ok(),
    };
    let ctx = &Ctx {
        home,
        access_token: scoped.or_else(|| std::env::var("KURA_ACCESS_TOKEN").ok()),
    };
    let result = match cli.command {
        Command::Login {
            server,
            issuer,
            client_id,
            device_name,
        } => commands::login(ctx, server, issuer, client_id, device_name).await,
        Command::Attach { vault, folder, yes } => commands::attach(ctx, &vault, folder, yes).await,
        Command::Sync {
            vault,
            dry_run,
            yes,
        } => commands::sync_cmd(ctx, vault.as_deref(), dry_run, yes).await,
        Command::Watch { vault } => {
            let stop = async {
                drop(tokio::signal::ctrl_c().await);
            };
            commands::watch(ctx, vault.as_deref(), stop).await
        }
        Command::Status => commands::status(ctx).await,
        Command::Verify { vault } => commands::verify(ctx, &vault).await,
        Command::History { vault, path } => commands::history(ctx, &vault, &path).await,
        Command::Restore {
            vault,
            path,
            revision,
        } => commands::restore(ctx, &vault, &path, revision).await,
        Command::Devices { action } => commands::devices(ctx, action).await,
        Command::Token { action } => commands::token(ctx, action).await,
        Command::Search { vault, query } => commands::search(ctx, &vault, &query).await,
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(Fail(code, msg)) => {
            eprintln!("error: {msg}");
            ExitCode::from(code)
        }
    }
}
