//! `superfik`: talk to Superfik's agent in a terminal.
//!
//! - `superfik [prompt]` opens the chat (sending `prompt` first).
//! - `superfik -p <prompt>` answers once and prints the reply.
//! - `superfik login` / `logout` connect or forget an LLM provider.
//! - `superfik models` lists what the connected providers offer.
//! - `superfik pull <model>` / `rm <model>` download or delete a local MLX model.
//! - `superfik mcp …` lists, adds, removes and approves MCP servers.
//!
//! Every terminal talks to one background daemon per computer (Unix), so
//! local models, embeddings and memory load once.

mod daemon;
mod local_models;
mod login;
mod mcp;
mod print;
mod tui;
mod update;
mod verdict;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use superfik_sdk::PermissionMode;

#[derive(Debug, Parser)]
#[command(name = "superfik", version, about = "Talk to Superfik's coding agent in your project.")]
struct Cli {
    /// What to ask; opens the chat with it unless --print is given.
    prompt: Vec<String>,
    /// Answer once, print the reply and exit.
    #[arg(short, long)]
    print: bool,
    /// Continue the last conversation in this folder.
    #[arg(short = 'c', long = "continue")]
    resume: bool,
    /// The model (its id or name); the last one used otherwise.
    #[arg(short, long)]
    model: Option<String>,
    /// ask, edits, plan or bypass.
    #[arg(long, value_parser = parse_mode)]
    mode: Option<PermissionMode>,
    /// With --print: judge the work against these criteria before it ends
    /// (exits 1 when it fails them).
    #[arg(long, requires = "print")]
    verification: Option<String>,
    /// The project folder (the current one by default).
    #[arg(short = 'C', long)]
    cwd: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Action>,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Connect an LLM provider: an API key, or a Claude or ChatGPT subscription.
    Login,
    /// Forget a provider's credentials.
    Logout,
    /// List the models the connected providers offer.
    Models,
    /// Download a local MLX model from Hugging Face (Apple silicon), e.g.
    /// `mlx-community/Qwen3-8B-4bit`; a bare name means mlx-community's.
    Pull { model: String },
    /// Delete a downloaded local MLX model.
    Rm { model: String },
    /// The MCP servers a session here can use: list, add, remove, approve.
    Mcp {
        #[command(subcommand)]
        action: Option<mcp::McpAction>,
    },
    /// Replace this binary with the latest release.
    Update,
    /// The background daemon every terminal shares.
    #[command(hide = true)]
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
}

#[derive(Debug, Subcommand)]
enum DaemonAction {
    /// Serve this computer's terminals until the last one leaves.
    Serve,
    /// Stop the running daemon (its terminals' runs end).
    Stop,
    /// Say whether one is running.
    Status,
}

fn parse_mode(text: &str) -> Result<PermissionMode, String> {
    superfik_core::command::mode_named(text).ok_or_else(|| format!("`{text}` is not ask, edits, plan or bypass"))
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("superfik: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode, String> {
    let cwd = match cli.cwd {
        Some(dir) => dir,
        None => std::env::current_dir().map_err(|e| e.to_string())?,
    };
    let cwd = std::fs::canonicalize(&cwd).map_err(|e| format!("{}: {e}", cwd.display()))?;
    #[cfg(unix)]
    if let Some(Action::Daemon { action }) = &cli.command {
        match action {
            DaemonAction::Serve => daemon::serve()?,
            DaemonAction::Stop => daemon::stop()?,
            DaemonAction::Status => println!("{}", daemon::status()),
        }
        return Ok(ExitCode::SUCCESS);
    }
    // Local models are this computer's files: no daemon needed.
    match &cli.command {
        Some(Action::Pull { model }) => return local_models::pull(model).map(|()| ExitCode::SUCCESS),
        Some(Action::Rm { model }) => return local_models::remove(model).map(|()| ExitCode::SUCCESS),
        Some(Action::Update) => return update::run().map(|()| ExitCode::SUCCESS),
        _ => {}
    }
    let (superfik, stale) = daemon::connect()?;
    let prompt = (!cli.prompt.is_empty()).then(|| cli.prompt.join(" "));
    match cli.command {
        Some(Action::Login) => login::login(&superfik).map(|()| ExitCode::SUCCESS),
        Some(Action::Logout) => login::logout(&superfik).map(|()| ExitCode::SUCCESS),
        Some(Action::Models) => {
            for model in superfik.models().map_err(|e| e.to_string())? {
                println!("{:<40} {}", model.id, model.label);
            }
            Ok(ExitCode::SUCCESS)
        }
        Some(Action::Mcp { action }) => {
            mcp::run(&superfik, &cwd, action.unwrap_or(mcp::McpAction::List)).map(|()| ExitCode::SUCCESS)
        }
        Some(Action::Daemon { .. }) => Err("the daemon needs macOS or Linux".into()),
        Some(Action::Pull { .. } | Action::Rm { .. } | Action::Update) => Ok(ExitCode::SUCCESS),
        None if cli.print => {
            if let Some(stale) = &stale {
                eprintln!("superfik: {stale}");
            }
            let prompt = prompt.ok_or("--print needs a prompt")?;
            let wanted = print::Wanted { model: cli.model.as_deref(), mode: cli.mode, verification: cli.verification };
            print::print(&superfik, cwd, wanted, &prompt)
        }
        None => {
            let resume = if cli.resume {
                let settings = superfik.settings().map_err(|e| e.to_string())?;
                let last = tui::last_session(&settings, &cwd);
                if last.is_none() {
                    eprintln!("superfik: no earlier conversation in this folder; starting a new one");
                }
                last
            } else {
                None
            };
            tui::run(superfik, cwd, cli.model, cli.mode, prompt, stale, resume).map_err(|e| e.to_string())?;
            Ok(ExitCode::SUCCESS)
        }
    }
}
